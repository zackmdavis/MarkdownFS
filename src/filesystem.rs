use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::PermissionsExt;

use fuse::{FileAttr, Filesystem, FileType, Request,
           ReplyAttr, ReplyDirectory, ReplyEntry};
use libc::ENOENT;
use time::Timespec;


// XXX CARGO CULT (no pun intended): what is a TTL in this context??
const TTL: Timespec = Timespec { sec: 1, nsec: 0 };


pub struct MarkdownFs {
    source_directory: PathBuf
}


impl MarkdownFs {
    pub fn new(path: &Path) -> Self {
        // TODO: verify that it's actually a directory
        MarkdownFs { source_directory: path.to_path_buf() }
    }
}

impl Filesystem for MarkdownFs {

    fn lookup(&mut self, _request: &Request,
              _parent: u64, name: &Path, reply: ReplyEntry) {
        info!("lookup {:?}", name);
        let sources = fs::read_dir(&self.source_directory).unwrap();
        match sources
            .map(|entry_result| { entry_result.unwrap().path() })
            // TODO: also verify .md extension?
            .find(|source_path| name == source_path) {
                Some(source_path) => {
                    let source_file = File::open(source_path).unwrap();
                    let source_metadata = source_file.metadata().unwrap();
                    let source_attributes = FileAttr {
                        ino: source_metadata.ino(),
                        size: source_metadata.size() as u64,
                        blocks: source_metadata.blocks() as u64,
                        atime: Timespec::new(source_metadata.atime(), 0),
                        mtime: Timespec::new(source_metadata.mtime(), 0),
                        ctime: Timespec::new(source_metadata.ctime(), 0),
                        crtime: Timespec::new(0, 0), // no servitude to Cupertino
                        kind: FileType::RegularFile,
                        perm: source_metadata.permissions().mode() as u16,
                        nlink: source_metadata.nlink() as u32,
                        uid: source_metadata.uid(),
                        gid: source_metadata.gid(),
                        rdev: source_metadata.rdev() as u32,
                        flags: 0, // no servitude to Cupertino
                    };
                    reply.entry(&TTL, &source_attributes, 0);
                },
                None => {
                    reply.error(ENOENT);
                }
            }
    }

    fn getattr(&mut self, _request: &Request, ino: u64, reply: ReplyAttr) {
        match ino {
            1 => {
                let root_metadata = fs::metadata(&self.source_directory)
                    .unwrap();
                let root_attributes = FileAttr {
                    ino: 1,
                    size: root_metadata.size() as u64,
                    blocks: root_metadata.blocks() as u64,
                    atime: Timespec::new(root_metadata.atime(), 0),
                    mtime: Timespec::new(root_metadata.mtime(), 0),
                    ctime: Timespec::new(root_metadata.ctime(), 0),
                    crtime: Timespec::new(0, 0), // no servitude to Cupertino
                    kind: FileType::Directory,
                    perm: root_metadata.permissions().mode() as u16,
                    nlink: root_metadata.nlink() as u32,
                    uid: root_metadata.uid(),
                    gid: root_metadata.gid(),
                    rdev: root_metadata.rdev() as u32,
                    flags: 0, // no servitude to Cupertino
                };
                reply.attr(&TTL, &root_attributes);
            },
            _ => {
                reply.error(ENOENT);
            }
        }
    }

    fn readdir(&mut self, _request: &Request,
               ino: u64, _fh: u64, offset: u64, mut reply: ReplyDirectory) {
        if ino == 1 {
            if offset == 0 {
                reply.add(1, 0, FileType::Directory, ".");
                reply.add(1, 1, FileType::Directory, "..");
                let sources = fs::read_dir(&self.source_directory).unwrap();

                let listitems = sources
                    .map(|entry_result| {
                        entry_result.unwrap().path().file_name().unwrap()
                            .to_str().unwrap().to_owned()
                    });
                for (i, listitem) in (2..).zip(listitems) {
                    reply.add(i, // XXX need real inode numbers probably
                              i,
                              FileType::RegularFile,
                              listitem);
                }
            }
            reply.ok();
        } else {
            reply.error(ENOENT);
        }
    }

}
