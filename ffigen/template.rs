#[repr(C)]
struct StrSlice {
    ptr: *const u8,
    len: usize,
}
