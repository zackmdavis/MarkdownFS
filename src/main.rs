extern crate fuse;


use std::env;

use fuse::Filesystem;


struct MarkdownFS;

impl Filesystem for MarkdownFS {}


fn main() {
    let mountpoint = env::args_os().nth(1).unwrap();
    fuse::mount(MarkdownFS, &mountpoint, &[]);
}
