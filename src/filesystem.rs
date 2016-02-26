use std::path::Path;

use fuse::{Filesystem, Request, ReplyEntry};
use libc::ENOENT;

pub struct MarkdownFs;

impl Filesystem for MarkdownFs {

    fn lookup(&mut self, _req: &Request,
              _parent: u64, name: &Path, reply: ReplyEntry) {
        info!("lookup {:?}", name);
        reply.error(ENOENT);
    }

}
