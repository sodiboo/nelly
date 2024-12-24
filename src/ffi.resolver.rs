#![allow(unused)]
use core::str;
use std::{
    ffi::{c_char, c_int, c_void, CStr},
    io::Write,
    path::Path,
    sync::LazyLock,
};

use elf::{endian::AnyEndian, note::Note, symbol::Symbol};

#[allow(non_camel_case_types)]
mod sys {
    use std::ffi::{c_char, c_int, c_void};

    #[repr(C)]
    pub struct _Dart_Handle {
        _unused: [u8; 0],
    }

    #[repr(C)]
    pub struct _Dart_NativeArguments {
        _unused: [u8; 0],
    }

    pub type Dart_Handle = *mut _Dart_Handle;
    pub type Dart_NativeArguments = *mut _Dart_NativeArguments;

    pub type Dart_FfiNativeResolver =
        Option<unsafe extern "C" fn(name: *const c_char, args_n: usize) -> *mut c_void>;
    pub type Dart_NativeFunction = Option<unsafe extern "C" fn(arguments: Dart_NativeArguments)>;
    pub type Dart_NativeEntrySymbol =
        Option<unsafe extern "C" fn(nf: Dart_NativeFunction) -> *const u8>;
    pub type Dart_NativeEntryResolver = Option<
        unsafe extern "C" fn(
            name: Dart_Handle,
            num_of_arguments: c_int,
            auto_setup_scope: *mut bool,
        ) -> Dart_NativeFunction,
    >;

    pub type Dart_NativeAssetsDlopenCallback =
        Option<unsafe extern "C" fn(path: *const c_char, error: *mut *mut c_char) -> *mut c_void>;
    pub type Dart_NativeAssetsDlopenCallbackNoPath =
        Option<unsafe extern "C" fn(error: *mut *mut c_char) -> *mut c_void>;
    pub type Dart_NativeAssetsDlsymCallback = Option<
        unsafe extern "C" fn(
            handle: *mut c_void,
            symbol: *const c_char,
            error: *mut *mut c_char,
        ) -> *mut c_void,
    >;

    pub struct NativeAssetsApi {
        pub dlopen_absolute: Dart_NativeAssetsDlopenCallback,
        pub dlopen_relative: Dart_NativeAssetsDlopenCallback,
        pub dlopen_system: Dart_NativeAssetsDlopenCallback,
        pub dlopen_process: Dart_NativeAssetsDlopenCallbackNoPath,
        pub dlopen_executable: Dart_NativeAssetsDlopenCallbackNoPath,
        pub dlsym: Dart_NativeAssetsDlsymCallback,
    }

    extern "C" {
        // this is used as a sentinel value to find the base address of the functions
        pub fn FlutterEngineGetCurrentTime() -> u64;
    }
}

use sys::*;
use tracing::debug;

struct FlutterEngineSymbols<'a> {
    symbols: elf::parse::ParsingTable<'a, AnyEndian, Symbol>,
    strings: elf::string_table::StringTable<'a>,
    base_addr: usize,
    provenance: *const (),
}

impl<'a> FlutterEngineSymbols<'a> {
    fn parse(libflutter_engine: &'a [u8]) -> Self {
        let file = elf::ElfBytes::<AnyEndian>::minimal_parse(libflutter_engine).unwrap();

        let (symbols, strings) = file
            .symbol_table()
            .expect("should have a symbol table")
            .expect("should have a symbol table");

        let provenance = FlutterEngineGetCurrentTime as *const ();

        let base_addr = symbols
            .iter()
            .find(|symbol| {
                strings
                    .get(symbol.st_name as usize)
                    .is_ok_and(|sym_name| sym_name == "FlutterEngineGetCurrentTime")
            })
            .map(|symbol| symbol.st_value as usize)
            .map(|offset| FlutterEngineGetCurrentTime as usize - offset)
            .expect("Symbol table should contain FlutterEngineGetCurrentTime");

        Self {
            symbols,
            strings,
            base_addr,
            provenance,
        }
    }

    fn get(&self, name: &str) -> Option<*const ()> {
        self.symbols
            .iter()
            .find(|symbol| {
                self.strings
                    .get(symbol.st_name as usize)
                    .is_ok_and(|sym_name| sym_name == name)
            })
            .map(|symbol| symbol.st_value as usize)
            .map(|offset| self.provenance.with_addr(self.base_addr + offset))
    }
}

static DART_SYMBOLS: LazyLock<BoundSymbols> = LazyLock::new(|| {
    let libflutter_engine_so =
        Path::new(crate::engine_meta::FLUTTER_ENGINE_PATH).join("libflutter_engine.so");
    debug!("libflutter_engine.so: {}", libflutter_engine_so.display());
    let libflutter_engine = std::fs::read(libflutter_engine_so).unwrap();

    let symbols = FlutterEngineSymbols::parse(&libflutter_engine);

    BoundSymbols::bind(&symbols)
});

pub fn warmup_dart_symbols() {
    tracing::debug_span!("warmup_dart_symbols").in_scope(|| {
        tracing::debug!("will warmup_dart_symbols");
        _ = *DART_SYMBOLS;
        tracing::debug!("did warmup_dart_symbols");
    });
}

macro_rules! dart_symbols {
    ($(
        fn $name:ident($($arg:ident: $arg_ty:ty),* $(,)?) $( -> $ret:ty)?;
    )*) => {
        $(
            #[allow(non_snake_case)]
            unsafe fn $name($($arg: $arg_ty),*) $(-> $ret)? {
                unsafe { (DART_SYMBOLS.$name)($($arg),*) }
            }
        )*

        #[allow(non_snake_case)]
        #[derive(Default)]
        struct MaybeSymbols {
            $(
                $name: Option<unsafe extern "C" fn($($arg: $arg_ty),*) $(-> $ret)?>,
            )*
        }

        impl MaybeSymbols {
            fn unwrap(self) -> BoundSymbols {
                BoundSymbols {
                    $(
                        $name: self.$name.expect(concat!("Symbol table should contain ", stringify!($name))),
                    )*
                }
            }

            fn visit(&mut self, name: &str, ptr: *const ()) {
                match name {
                    $(
                        stringify!($name) => self.$name = Some(unsafe { std::mem::transmute(ptr) }),
                    )*
                    _ => {}
                }
            }
        }

        #[allow(non_snake_case)]
        struct BoundSymbols {
            $(
                $name: unsafe extern "C" fn($($arg: $arg_ty),*) $(-> $ret)?,
            )*
        }
    };
}

impl BoundSymbols {
    fn bind(source: &FlutterEngineSymbols) -> Self {
        let mut maybe_symbols = MaybeSymbols::default();

        for sym in source.symbols.iter() {
            if let Ok(sym_name) = source.strings.get(sym.st_name as usize) {
                let ptr = source
                    .provenance
                    .with_addr(source.base_addr + sym.st_value as usize);
                maybe_symbols.visit(sym_name, ptr);
            }
        }

        maybe_symbols.unwrap()
    }
}

dart_symbols! {
    fn Dart_RootLibrary() -> Dart_Handle;
    fn Dart_LibraryUrl(library: Dart_Handle) -> Dart_Handle;
    fn Dart_SetFfiNativeResolver(library: Dart_Handle, resolver: Dart_FfiNativeResolver) -> Dart_Handle;
    fn Dart_StringToCString(str: Dart_Handle, cstr: *mut *const ::core::ffi::c_char) -> Dart_Handle;
    fn Dart_SetNativeResolver(library: Dart_Handle, resolver: Dart_NativeEntryResolver, symbol: Dart_NativeEntrySymbol) -> Dart_Handle;

    fn Dart_InitializeNativeAssetsResolver( native_assets_api: *const NativeAssetsApi);

    fn Dart_Null() -> Dart_Handle;
    fn Dart_IsNull(handle: Dart_Handle) -> bool;
    fn Dart_IsError(handle: Dart_Handle) -> bool;
    fn Dart_EnterScope();
    fn Dart_ExitScope();
}

#[allow(non_snake_case)]
pub extern "C" fn init_resolver() {
    tracing::warn!("init_resolver call");
    unsafe {
        let root_library = Dart_RootLibrary();

        if root_library.is_null() {
            tracing::error!("root_library is null");
        } else if root_library == Dart_Null() {
            tracing::error!("root_library is Dart_Null");
        } else if Dart_IsNull(root_library) {
            tracing::error!("root_library is Dart_IsNull");
        } else {
            tracing::info!("root_library is {:x?}", root_library.addr());
        }

        let library_uri = Dart_LibraryUrl(root_library);

        let mut cstr = std::ptr::null();
        let ret = Dart_StringToCString(library_uri, &mut cstr);
        if Dart_IsError(ret) {
            tracing::error!("Dart_StringToCString failed");
        } else {
            let cstr = CStr::from_ptr(cstr);
            let cstr = cstr.to_str().unwrap();
            tracing::info!("library_uri: {cstr}");
        }

        // tracing::debug!("Dart_CObject: {Dart_PostCObject:?}");
        // tracing::debug!("actual: {data:?}");

        let ret: Dart_Handle = Dart_SetFfiNativeResolver(root_library, Some(ffi_native_resolver));

        if Dart_IsError(ret) {
            tracing::error!("Dart_SetFfiNativeResolver failed");
        }

        let ret: Dart_Handle = Dart_SetNativeResolver(
            root_library,
            Some(native_entry_resolver),
            Some(native_symbol_resolver),
        );

        if Dart_IsError(ret) {
            tracing::error!("Dart_SetNativeResolver failed");
        }

        let native_assets_api = NativeAssetsApi {
            dlopen_absolute: Some(dlopen_absolute),
            dlopen_relative: Some(dlopen_relative),
            dlopen_system: Some(dlopen_system),
            dlopen_process: Some(dlopen_process),
            dlopen_executable: Some(dlopen_executable),
            dlsym: Some(dlsym),
        };

        let () = Dart_InitializeNativeAssetsResolver(&raw const native_assets_api);
    }
    tracing::warn!("init_resolver exit");
}

extern "C" fn dlopen_absolute(path: *const c_char, error: *mut *mut c_char) -> *mut c_void {
    let path = unsafe { CStr::from_ptr(path) };
    let path = path.to_str().unwrap();
    tracing::debug!("dlopen_absolute: {path}");
    std::ptr::dangling_mut()
}
extern "C" fn dlopen_relative(path: *const c_char, error: *mut *mut c_char) -> *mut c_void {
    let path = unsafe { CStr::from_ptr(path) };
    let path = path.to_str().unwrap();
    tracing::debug!("dlopen_relative: {path}");
    std::ptr::dangling_mut()
}
extern "C" fn dlopen_system(path: *const c_char, error: *mut *mut c_char) -> *mut c_void {
    let path = unsafe { CStr::from_ptr(path) };
    let path = path.to_str().unwrap();
    tracing::debug!("dlopen_system: {path}");
    std::ptr::dangling_mut()
}
extern "C" fn dlopen_process(error: *mut *mut c_char) -> *mut c_void {
    tracing::debug!("dlopen_process");
    std::ptr::dangling_mut()
}
extern "C" fn dlopen_executable(error: *mut *mut c_char) -> *mut c_void {
    tracing::debug!("dlopen_executable");
    std::ptr::dangling_mut()
}
extern "C" fn dlsym(
    handle: *mut c_void,
    symbol: *const c_char,
    error: *mut *mut c_char,
) -> *mut c_void {
    let symbol = unsafe { CStr::from_ptr(symbol) };
    let symbol = symbol.to_str().unwrap();
    tracing::debug!("dlsym: {symbol}");
    std::ptr::null_mut()
}

pub fn post_init() {
    tracing::debug!("post_init at thread {:?}", std::thread::current().id());
}

unsafe extern "C" fn ffi_native_resolver(
    name: *const c_char,
    args_n: usize,
) -> *mut std::ffi::c_void {
    tracing::warn!("ffi native resolver call");
    let name = CStr::from_ptr(name);

    let name = name.to_str().unwrap();

    tracing::info!("ffi native resolver: {name}({args_n})");
    std::ptr::null_mut()
}

unsafe extern "C" fn native_entry_resolver(
    name: Dart_Handle,
    num_of_arguments: c_int,
    auto_setup_scope: *mut bool,
) -> Dart_NativeFunction {
    let mut cstr = std::ptr::null();
    let ret = Dart_StringToCString(name, &raw mut cstr);
    if Dart_IsError(ret) {
        tracing::error!("Dart_StringToCString failed");
        return None;
    } else {
        let cstr = CStr::from_ptr(cstr);
        let cstr = cstr.to_str().unwrap();
        tracing::info!("native entry resolver: {cstr}({num_of_arguments})");
    }

    None
}

unsafe extern "C" fn native_symbol_resolver(nf: Dart_NativeFunction) -> *const u8 {
    tracing::warn!("native symbol resolver call");
    std::ptr::null()
}
