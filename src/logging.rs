use std::io;
use std::io::Write;

use log;
use time;


pub struct MarkdownFsLogger;

impl log::Log for MarkdownFsLogger {
    fn enabled(&self, _metadata: &log::LogMetadata) -> bool { true }

    fn log(&self, record: &log::LogRecord) {
        writeln!(&mut io::stderr(),
                 "[{}] {}",
                 time::now().strftime("%Y-%m-%d %H:%M:%S.%f").unwrap(),
                 record.args())
            .expect("couldn't write to stderr?!");
    }
}
