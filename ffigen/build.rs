use std::{borrow::Cow, fs::File, io::Write, path::Path};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum FfiType {
    Bool,

    // Isize, // Dart has no ffi.SignedSize???
    Usize,

    U8,
    U16,
    U32,
    U64,

    I8,
    I16,
    I32,
    I64,

    F32,
    F64,

    Str,
}

impl FfiType {
    // fn to_dart_ffi(&self) -> Cow<'static, str> {
    //     match self {
    //         FfiType::Bool => "ffi.Bool".into(),

    //         FfiType::Usize => "ffi.Size".into(),

    //         FfiType::U8 => "ffi.Uint8".into(),
    //         FfiType::U16 => "ffi.Uint16".into(),
    //         FfiType::U32 => "ffi.Uint32".into(),
    //         FfiType::U64 => "ffi.Uint64".into(),

    //         FfiType::I8 => "ffi.Int8".into(),
    //         FfiType::I16 => "ffi.Int16".into(),
    //         FfiType::I32 => "ffi.Int32".into(),
    //         FfiType::I64 => "ffi.Int64".into(),

    //         FfiType::F32 => "ffi.Float".into(),
    //         FfiType::F64 => "ffi.Double".into(),

    //         FfiType::Str => "SliceStr".into(),
    //     }
    // }

    // fn to_dart(&self) -> Cow<'static, str> {
    //     match self {
    //         FfiType::Bool => "bool".into(),

    //         FfiType::Usize => "int".into(),

    //         FfiType::U8 => "int".into(),
    //         FfiType::U16 => "int".into(),
    //         FfiType::U32 => "int".into(),
    //         FfiType::U64 => "int".into(),

    //         FfiType::I8 => "int".into(),
    //         FfiType::I16 => "int".into(),
    //         FfiType::I32 => "int".into(),
    //         FfiType::I64 => "int".into(),

    //         FfiType::F32 => "double".into(),
    //         FfiType::F64 => "double".into(),

    //         FfiType::Str => "SliceStr".into(),
    //     }
    // }

    // fn to_rust_ffi(&self) -> Cow<'static, str> {
    //     match self {
    //         FfiType::Bool => "bool".into(),

    //         FfiType::Usize => "usize".into(),

    //         FfiType::U8 => "u8".into(),
    //         FfiType::U16 => "u16".into(),
    //         FfiType::U32 => "u32".into(),
    //         FfiType::U64 => "u64".into(),

    //         FfiType::I8 => "i8".into(),
    //         FfiType::I16 => "i16".into(),
    //         FfiType::I32 => "i32".into(),
    //         FfiType::I64 => "i64".into(),

    //         FfiType::F32 => "f32".into(),
    //         FfiType::F64 => "f64".into(),

    //         FfiType::Str => "StrSlice".into(),
    //     }
    // }

    // fn from_syn_type(ty: &syn::Type) -> Option<FfiType> {
    //     match ty {
    //         syn::Type::Array(_) => None,
    //         syn::Type::BareFn(_) => None,
    //         syn::Type::Group(ty) => Self::from_syn_type(&ty.elem),
    //         syn::Type::ImplTrait(_) => None,
    //         syn::Type::Infer(_) => None,
    //         syn::Type::Macro(_) => None,
    //         syn::Type::Never(_) => None,
    //         syn::Type::Paren(ty) => Self::from_syn_type(&ty.elem),
    //         syn::Type::Path(ty) => {
    //             if ty.qself.is_some() {
    //                 None
    //             } else {
    //                 let segments = &ty.path.segments;
    //                 if segments.len() != 1 {
    //                     return None;
    //                 }

    //                 let segment = &segments[0];
    //                 if !segment.arguments.is_empty() {
    //                     return None;
    //                 }

    //                 let ident = &segment.ident;

    //                 match ident.to_string().as_str() {
    //                     "bool" => Some(FfiType::Bool),
    //                     "usize" => Some(FfiType::Usize),

    //                     "u8" => Some(FfiType::U8),
    //                     "u16" => Some(FfiType::U16),
    //                     "u32" => Some(FfiType::U32),
    //                     "u64" => Some(FfiType::U64),

    //                     "i8" => Some(FfiType::I8),
    //                     "i16" => Some(FfiType::I16),
    //                     "i32" => Some(FfiType::I32),
    //                     "i64" => Some(FfiType::I64),

    //                     "f32" => Some(FfiType::F32),
    //                     "f64" => Some(FfiType::F64),
    //                     _ => None,
    //                 }
    //             }
    //         }
    //         syn::Type::Ptr(_) => None,
    //         syn::Type::Reference(ty) => {
    //             if ty.mutability.is_some() {
    //                 None
    //             } else {
    //                 match &ty.elem {
    //                     syn::Type::Slice()
    //                 }
    //             }
    //         }
    //         syn::Type::Slice(ty) => todo!(),
    //         syn::Type::TraitObject(ty) => todo!(),
    //         syn::Type::Tuple(ty) => todo!(),
    //         syn::Type::Verbatim(ty) => todo!(),
    //         _ => todo!(),
    //     }
    // }

    // fn from_syn_ref(ty: &syn::TypeReference) -> Option<FfiType> {
    //     match &ty.elem {
    //         syn::Type::Path(ty) => {}
    //         _ => None,
    //     }
    // }
}

enum ReturnType {
    Void,
    Type(FfiType),
}

pub fn generate_glue() {
    let Ok(input) = syn::parse_file(include_str!("../src/ffi.rs")) else {
        // parse errors will prevent the build from succeeding anyway
        return;
    };

    let cargo_manifest_dir = super::env("CARGO_MANIFEST_DIR").unwrap();

    let out_dir = super::env("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("ffi.gen.rs");

    let mut f = File::create(out_path).unwrap();

    let template = cargo_manifest_dir + "/ffigen/template.rs";

    writeln!(f, r"include!({template:?});",).unwrap();

    // for item in input.items {
    //     if let syn::Item::Fn(item) = item {
    //         if item.sig.asyncness.is_some() {
    //             println!(
    //                 "cargo::error::async fn {}(...): async functions are not supported",
    //                 item.sig.ident
    //             );
    //             continue;
    //         }

    //         if item.sig.variadic.is_some() {
    //             println!(
    //                 "cargo::error::fn {}(...): variadic functions are not supported",
    //                 item.sig.ident
    //             );
    //             continue;
    //         }

    //         if !item.sig.generics.params.is_empty() {
    //             println!(
    //                 "cargo::error::fn {}::<...>(...): generic functions are not supported",
    //                 item.sig.ident
    //             );
    //             continue;
    //         }

    //         if item.sig.abi.is_some() {
    //             println!(
    //                 "cargo::error::extern fn {}(...): functions with custom ABIs are not supported",
    //                 item.sig.ident
    //             );
    //             continue;
    //         }

    //         // let return_type = match item.sig.output {
    //         //     syn::ReturnType::Default => ReturnType::Void,
    //         //     syn::ReturnType::Type(_, ty) => match ty.as_ref() {
    //         //         syn::Type::Array(ty) => todo!(),
    //         //         syn::Type::BareFn(ty) => todo!(),
    //         //         syn::Type::Group(ty) => todo!(),
    //         //         syn::Type::ImplTrait(ty) => todo!(),
    //         //         syn::Type::Infer(ty) => todo!(),
    //         //         syn::Type::Macro(ty) => todo!(),
    //         //         syn::Type::Never(ty) => todo!(),
    //         //         syn::Type::Paren(ty) => todo!(),
    //         //         syn::Type::Path(ty) => todo!(),
    //         //         syn::Type::Ptr(ty) => todo!(),
    //         //         syn::Type::Reference(ty) => todo!(),
    //         //         syn::Type::Slice(ty) => todo!(),
    //         //         syn::Type::TraitObject(ty) => todo!(),
    //         //         syn::Type::Tuple(ty) => todo!(),
    //         //         syn::Type::Verbatim(ty) => todo!(),
    //         //         _ => todo!(),
    //         //     },
    //         // };
    //     }
    // }
}
