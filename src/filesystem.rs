use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use fuse::{FileAttr, Filesystem, FileType, Request,
           ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry};
use libc::{EIO, ENOENT};
use time::Timespec;


// XXX CARGO CULT (no pun intended): what is a TTL in this context??
const TTL: Timespec = Timespec { sec: 1, nsec: 0 };


fn hoist_attributes(ino: u64, backing_path: &Path)
                    -> Result<FileAttr, io::Error> {
    info!("hoisting attributes for {:?} as ino {}", backing_path, ino);
    let source_file = try!(File::open(backing_path));
    let kind = if backing_path.is_dir() {
        FileType::RegularFile
    } else if backing_path.is_file() {
        FileType::Directory
    } else {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("path {:?} is not to a directory or regular file",
                    backing_path)));
    };
    let source_metadata = try!(source_file.metadata());
    let source_attributes = FileAttr {
        ino: ino,
        size: source_metadata.size() as u64,
        blocks: source_metadata.blocks() as u64,
        atime: Timespec::new(source_metadata.atime(), 0),
        mtime: Timespec::new(source_metadata.mtime(), 0),
        ctime: Timespec::new(source_metadata.ctime(), 0),
        crtime: Timespec::new(0, 0), // no servitude to Cupertino
        kind: kind,
        perm: source_metadata.permissions().mode() as u16,
        nlink: source_metadata.nlink() as u32,
        uid: source_metadata.uid(),
        gid: source_metadata.gid(),
        rdev: source_metadata.rdev() as u32,
        flags: 0, // no servitude to Cupertino
    };
    Ok(source_attributes)
}


#[derive(Debug)]
pub struct MarkdownFs {
    // It seems like a lot of work to actually maintain substantive inode
    // structures for such a puny read-only "filter" filesystem ("filesystem"?)
    // that passes through to a real filesystem to actually get the
    // information. But the FUSE API seems to expect to get and receive
    // inode-numbers for a lot of things, so ... we'll just keep track of which
    // inos correspond to which paths, and vice-versa?
    source_directory: PathBuf,
    next_ino: u64,
    // someday someone should write a bidirectional map crate
    //
    // between now and then, should these one-way maps share an
    // Rc<PathBuf>??—as suggested at http://stackoverflow.com/a/33616960
    ino_to_backing_path: HashMap<u64, PathBuf>,
    backing_path_to_ino: HashMap<PathBuf, u64>
}


impl MarkdownFs {
    pub fn new(source_directory: &Path) -> Self {
        if !source_directory.is_dir() {
            panic!("MarkdownFS source root needs to be a directory, got {:?}",
                   source_directory);
        }
        let mut ino_to_backing_path = HashMap::new();
        ino_to_backing_path.insert(1, source_directory.to_path_buf());
        let mut backing_path_to_ino = HashMap::new();
        backing_path_to_ino.insert(source_directory.to_path_buf(), 1);
        let first_nonroot_ino = 2;

        MarkdownFs { source_directory: source_directory.to_path_buf(),
                     next_ino: first_nonroot_ino,
                     ino_to_backing_path: ino_to_backing_path,
                     backing_path_to_ino: backing_path_to_ino }
    }

    pub fn ino(&self, backing_path: &Path) -> Option<&u64> {
        self.backing_path_to_ino.get(backing_path)
    }

    pub fn backing_path(&self, ino: u64) -> Option<&PathBuf> {
        self.ino_to_backing_path.get(&ino)
    }
}


impl Filesystem for MarkdownFs {

    fn lookup(&mut self, request: &Request,
              parent: u64, name: &Path, reply: ReplyEntry) {
        info!("lookup request: {:?}; parent: {}, name: {:?}",
              request, parent, name);
        // `lookup` should "return" file attributes including an ino. If we
        // don't already know the ino, we need to make a new one.
        let mut full_path = PathBuf::new();
        let parent_subpath = match self.backing_path(parent).cloned() {
            Some(path) => path,
            None => {
                reply.error(ENOENT);
                return;
            }
        };
        full_path.push(parent_subpath);
        full_path.push(name);
        let ino_maybe = self.ino(&full_path).cloned();
        let ino = match ino_maybe {
            Some(our_ino) => our_ino,
            None => {
                let our_ino = self.next_ino;
                self.ino_to_backing_path.insert(our_ino, full_path.clone());
                self.backing_path_to_ino.insert(full_path.clone(), our_ino);
                self.next_ino += 1;
                our_ino
            }
        };
        match hoist_attributes(ino, &full_path) {
            Ok(attrs) => {
                reply.entry(&TTL, &attrs, 0);
            },
            Err(_) => {
                reply.error(ENOENT);
            }
        }
    }

    fn getattr(&mut self, request: &Request, ino: u64, reply: ReplyAttr) {
        info!("getattr request: {:?}; ino: {:?}", request, ino);
        match self.backing_path(ino) {
            Some(path) => {
                reply.attr(&TTL, &hoist_attributes(ino, path).unwrap());
            },
            None => {
                reply.error(ENOENT);
            }
        }
    }

    fn read(&mut self, request: &Request, ino: u64,
            fh: u64, offset: u64, size: u32, reply: ReplyData) {
        info!("read request: {:?}; ino: {:?}; fh: {:?}", request, ino, fh);
        match self.backing_path(ino) {
            Some(path) => {
                let file = match File::open(path) {
                    Ok(our_file) => our_file,
                    Err(_) => {
                        reply.error(EIO);
                        return;
                    }
                };
                let mut buffer: Vec<u8> = Vec::with_capacity(size as usize);
                let bytes_read = file.take(size as u64).read(&mut buffer)
                    .expect("couldn't read file?!");
                info!("read {} bytes from {:?}", bytes_read, path);
                // TODO: render Markdown
                reply.data(&buffer[offset as usize..]);
            },
            None => {
                reply.error(ENOENT);
            }
        }
    }

    fn readdir(&mut self, request: &Request,
               ino: u64, _fh: u64, _offset: u64, mut reply: ReplyDirectory) {
        info!("readdir request: {:?}; ino: {:?}", request, ino);
        let directory_path_maybe = self.backing_path(ino).cloned();
        let directory_path = match directory_path_maybe {
            Some(path) => path,
            None => {
                reply.error(ENOENT);
                return;
            }
        };
        let sources = match fs::read_dir(&directory_path) {
            Ok(listing) => listing,
            Err(_) => {
                reply.error(EIO);
                return;
            }
        };
        reply.add(ino, 0, FileType::Directory, ".");
        reply.add(ino, // XXX WRONG
                  1, FileType::Directory, "..");
        // TODO: use `offset`
        for (index, entry_result) in (2..).zip(sources) {
            if let Ok(entry) = entry_result {
                let path = entry.path();
                // XXX TODO: yank this and similar (copy-pasted from) code in
                // `lookup` into own method
                let ino_maybe = self.ino(&path).cloned();
                let ino = match ino_maybe {
                    Some(our_ino) => our_ino,
                    None => {
                        let our_ino = self.next_ino;
                        self.ino_to_backing_path
                            .insert(our_ino, path.clone());
                        self.backing_path_to_ino
                            .insert(path.clone(), our_ino);
                        self.next_ino += 1;
                        our_ino
                    }
                };
                // XXX more-ish code duplication
                let kind = if entry.file_type().unwrap().is_dir() {
                    FileType::RegularFile
                } else if entry.file_type().unwrap().is_file() {
                    FileType::Directory
                } else {
                    panic!("path {:?} is not to a directory or regular file",
                           path);
                };
                reply.add(ino, index, kind, path);
            }
        }
        reply.ok();
    }

}
