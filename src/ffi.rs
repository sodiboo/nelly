use core::str;
use std::{
    ffi::{c_char, CStr},
    io::Write,
    path::Path,
    ptr::{from_raw_parts, Pointee},
};

use elf::{endian::AnyEndian, note::Note, symbol::Symbol};

// mod gen {
//     include!(concat!(env!("OUT_DIR"), "/ffi.gen.rs"));
// }

// #[path = "ffi.resolver.rs"]
// mod resolver;

// pub(crate) use resolver::{init_resolver, post_init, warmup_dart_symbols};

pub trait FfiParam {
    type Foreign;

    fn from_foreign(foreign: Self::Foreign) -> Self;
}

pub trait FfiReturn {
    type Foreign;

    fn into_foreign(self) -> Self::Foreign;
}

macro_rules! ffi_transparent {
    {$($ty:ty),*$(,)?} => {$(
        impl FfiParam for $ty {
            type Foreign = Self;
            fn from_foreign(v: Self) -> Self {
                v
            }
        }

        impl FfiReturn for $ty {
            type Foreign = Self;
            fn into_foreign(self) -> Self {
                self
            }
        }
    )*}
}

ffi_transparent! {
    (), bool, f32, f64,
    i8, i16, i32, i64, isize,
    u8, u16, u32, u64, usize,
}

impl FfiParam for &str {
    type Foreign = *const c_char;
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn from_foreign(foreign: Self::Foreign) -> Self {
        unsafe { CStr::from_ptr(foreign) }.to_str().unwrap()
    }
}

#[repr(C)]
pub struct ByteSlice
where
    [u8]: Pointee<Metadata = usize>,
{
    ptr: *mut u8,
    len: <[u8] as Pointee>::Metadata,
}

impl FfiParam for *mut [u8] {
    type Foreign = ByteSlice;

    fn from_foreign(foreign: Self::Foreign) -> Self {
        std::ptr::from_raw_parts_mut(foreign.ptr, foreign.len)
    }
}

impl FfiReturn for *mut [u8] {
    type Foreign = ByteSlice;

    fn into_foreign(self) -> Self::Foreign {
        let (ptr, len) = self.to_raw_parts();
        ByteSlice {
            ptr: ptr.cast(),
            len,
        }
    }
}

macro_rules! nelly_ffi {
    {fn $fn:ident($($arg:ident: $ty:ty),*$(,)?)} => {
        nelly_ffi!(fn $fn($($arg: $ty),*) -> ());
    };
    {fn $fn:ident($($arg:ident: $ty:ty),*$(,)?) -> $ret:ty} => {
        #[export_name = concat!("nelly_ffi_", stringify!($fn))]
        #[expect(clippy::missing_safety_doc)]
        pub unsafe extern "C" fn $fn($($arg: <$ty as super::FfiParam>::Foreign),*)-> <$ret as super::FfiReturn>::Foreign {
            $(
                let $arg = <$ty as super::FfiParam>::from_foreign($arg);
            )*
            let ret = super::$fn($($arg),*);
            <$ret as super::FfiReturn>::into_foreign(ret)
        }
    }
}

macro_rules! ffi_fns {
    {
        $(
            $(#[$meta:meta])*
            $pub:vis fn $fn:ident($($arg:ident: $ty:ty),*$(,)?)$( -> $ret:ty)? $body:block
        )*
    } => {
        #[allow(clippy::must_use_candidate)]
        pub mod gen {
            $(nelly_ffi!(fn $fn($($arg: $ty),*)$(-> $ret)?);)*
        }

        $(
            $(#[$meta])*
            #[allow(clippy::must_use_candidate, reason = "it's all ffi. doesn't matter.")]
            #[allow(clippy::not_unsafe_ptr_arg_deref, reason = "safety is meh on ffi")]
            $pub fn $fn($($arg: $ty),*)$(-> $ret)? $body
        )*
    }
}

#[allow(
    clippy::must_use_candidate,
    clippy::missing_safety_doc,
    clippy::missing_panics_doc
)]
pub mod gen {

    // #[no_mangle]
    // pub unsafe extern "C" fn nelly_ffi_alloc_slice(bytes: usize) -> ByteSlice {
    //     let slice = Box::into_raw(vec![0; bytes].into_boxed_slice());
    //     slice.into()
    // }
    // #[no_mangle]
    // pub unsafe extern "C" fn nelly_ffi_free_slice(
    //     slice: <*mut [u8] as super::FfiParam>::Foreign,
    // ) -> <() as super::FfiReturn>::Foreign {
    //     let slice = <*mut [u8] as super::FfiParam>::from_foreign(slice);
    //     let ret = super::free_slice(slice);
    //     <() as super::FfiReturn>::into_foreign(ret)
    // }
    #[no_mangle]
    pub unsafe extern "C" fn nelly_ffi_log(
        level: usize,
        line: u32,

        target: *const u8,
        target_len: usize,

        file: *const u8,
        file_len: usize,

        msg: *const u8,
        msg_len: usize,
    ) {
        let target = std::slice::from_raw_parts(target, target_len);
        let file = std::slice::from_raw_parts(file, file_len);
        let msg = std::slice::from_raw_parts(msg, msg_len);

        let target = std::str::from_utf8(target).unwrap();
        let file = std::str::from_utf8(file).unwrap();
        let msg = std::str::from_utf8(msg).unwrap();

        super::log(level, target, file, line, msg)
    }
    #[no_mangle]
    pub unsafe extern "C" fn nelly_ffi_println(msg: *const u8, len: usize) {
        let msg = std::slice::from_raw_parts(msg, len);
        let msg = std::str::from_utf8(msg).unwrap();

        super::println(msg)
    }
}

// pub fn alloc_slice(bytes: usize) -> *mut [u8] {
//     Box::into_raw(vec![0; bytes].into_boxed_slice())
// }
// pub fn free_slice(slice: *mut [u8]) {
//     let abox = unsafe { Box::from_raw(slice) };
//     drop(abox);
// }

pub fn log(level: usize, target: &str, file: &str, line: u32, msg: &str) {
    let level = match level {
        1 => log::Level::Error,
        2 => log::Level::Warn,
        3 => log::Level::Info,
        4 => log::Level::Debug,
        5 => log::Level::Trace,
        _ => unreachable!("invalid log level"),
    };
    ::log::logger().log(
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

#[expect(clippy::print_stdout, reason = "this is a logging function")]
fn println(msg: &str) {
    println!("{msg}");
}
