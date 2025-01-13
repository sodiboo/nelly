#![warn(clippy::pedantic)]

mod ffi;

fn log(level: usize, target: &str, file: &str, line: u32, msg: &str) {
    let level = match level {
        1 => log::Level::Error,
        2 => log::Level::Warn,
        3 => log::Level::Info,
        4 => log::Level::Debug,
        5 => log::Level::Trace,
        _ => unreachable!("invalid log level"),
    };
    log::logger().log(
        &log::Record::builder()
            .target(target)
            .args(format_args!("{msg}"))
            .level(level)
            .module_path_static(Some(std::module_path!()))
            .file(Some(file))
            .line(Some(line))
            .build(),
    );
}

fn println(msg: &str) {
    println!("{msg}");
}

pub fn log_info_with_tag(tag: &str, msg: &str) {
    ::log::info!(target: &tag, "{msg}");
}
