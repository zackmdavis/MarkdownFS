extern crate fuse;
#[macro_use] extern crate log;
extern crate libc;
extern crate time;


mod logging;
mod filesystem;


use std::env;

use log::LogLevelFilter;

use filesystem::MarkdownFs;
use logging::MarkdownFsLogger;


fn main() {
    let source_directory = env::args_os().nth(1).unwrap();
    let mountpoint = env::args_os().nth(2).unwrap();

    log::set_logger(|max_log_level| {
        max_log_level.set(LogLevelFilter::Info);
        Box::new(MarkdownFsLogger)
    }).expect("couldn't initialize logging?!");

    fuse::mount(MarkdownFs, &mountpoint, &[]);
}
