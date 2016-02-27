use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::PermissionsExt;

use fuse::{FileAttr, Filesystem, FileType, Request,
           ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry};
use libc::ENOENT;
use time::Timespec;


// XXX CARGO CULT (no pun intended): what is a TTL in this context??
const TTL: Timespec = Timespec { sec: 1, nsec: 0 };

#[derive(Debug)]
pub struct Inode {
    ino: u64,
    truepath: PathBuf,
    attributes: FileAttr
}

impl Inode {
    fn reify(ino: u64, truepath: &Path) -> Self {
        info!("reifying inode {} for truepath {:?}", ino, truepath);
        let source_file = File::open(truepath).unwrap();
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
        Inode { ino: ino,
                truepath: truepath.to_path_buf(),
                attributes: source_attributes }
    }
}


#[derive(Debug)]
pub struct MarkdownFs {
    source_directory: PathBuf,
    inodes: Vec<Inode>
}


impl MarkdownFs {
    pub fn new(source_directory: &Path) -> Self {
        // TODO: verify that path is actually a directory
        let root_metadata = fs::metadata(source_directory).unwrap();
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
        let root_node = Inode {
            ino: 1,
            truepath: source_directory.to_path_buf(),
            attributes: root_attributes
        };
        MarkdownFs { source_directory: source_directory.to_path_buf(),
                     inodes: vec![root_node] }
    }

    pub fn assimilate(&mut self, truepath: &Path) -> u64 {
        info!("assimilating {:?} as inode", truepath);
        let new_ino = (self.inodes.len() + 1) as u64;
        self.inodes.push(Inode::reify(new_ino, truepath));
        self.inodes.len() as u64
    }

    pub fn inode(&self, ino: u64) -> Option<&Inode> {
        info!("fetching inode {:?} of {:?}", ino, self.inodes.len());
        let index = (ino - 1) as usize;
        self.inodes.get(index)
    }
}


impl Filesystem for MarkdownFs {

    fn lookup(&mut self, request: &Request,
              _parent: u64, name: &Path, reply: ReplyEntry) {
        info!("lookup request: {:?}; name: {:?}", request, name);
        let sources = fs::read_dir(&self.source_directory).unwrap();
        match sources
            .map(|entry_result| { entry_result.unwrap().path() })
            // TODO: also verify .md extension?
            .find(|source_path| {
                name.as_os_str() == source_path.file_name().unwrap()
            }) {
                Some(source_path) => {
                    let ino = self.assimilate(&source_path);
                    reply.entry(&TTL, &self.inode(ino).unwrap().attributes, 0);
                },
                None => {
                    reply.error(ENOENT);
                }
            }
    }

    fn getattr(&mut self, request: &Request, ino: u64, reply: ReplyAttr) {
        info!("getattr request: {:?}; ino: {:?}", request, ino);
        match self.inode(ino) {
            Some(inode) => {
                reply.attr(&TTL, &inode.attributes)
            },
            None => {
                reply.error(ENOENT);
            }
        }
    }

    fn read(&mut self, request: &Request, ino: u64,
            fh: u64, offset: u64, size: u32, reply: ReplyData) {
        info!("read request: {:?}; ino: {:?}; fh: {:?}", request, ino, fh);

    }

    fn readdir(&mut self, request: &Request,
               ino: u64, _fh: u64, offset: u64, mut reply: ReplyDirectory) {
        info!("readdir request: {:?}; ino: {:?}", request, ino);
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
