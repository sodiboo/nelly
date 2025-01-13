#[unsafe(no_mangle)]
pub unsafe extern "C" fn dart_tracing_log(
    level: usize,
    line: u32,

    target: *const u8,
    target_len: usize,

    file: *const u8,
    file_len: usize,

    msg: *const u8,
    msg_len: usize,
) {
    let target = unsafe { std::slice::from_raw_parts(target, target_len) };
    let file = unsafe { std::slice::from_raw_parts(file, file_len) };
    let msg = unsafe { std::slice::from_raw_parts(msg, msg_len) };

    let target = std::str::from_utf8(target).unwrap();
    let file = std::str::from_utf8(file).unwrap();
    let msg = std::str::from_utf8(msg).unwrap();

    super::log(level, target, file, line, msg);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dart_tracing_println(msg: *const u8, len: usize) {
    let msg = unsafe { std::slice::from_raw_parts(msg, len) };

    let msg = std::str::from_utf8(msg).unwrap();

    super::println(msg);
}
