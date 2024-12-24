use std::{
    ffi::CStr,
    io::{Read, Result, Seek, Write},
    sync::atomic::{AtomicI64, AtomicU64, Ordering},
};

use fluster::ViewId;

use crate::{
    embedder::FlutterWaylandSurface,
    nelly::Nelly,
    shell::{xdg::window::WindowDecorations, WaylandSurface},
};

mod binary;
mod xdg_toplevel;

use self::binary::{BinaryReader, BinaryWriter};
pub use self::xdg_toplevel::{CreateXdgToplevel, RemoveXdgToplevel, UpdateXdgToplevel};

trait RawPlatformMessage: std::fmt::Debug + Sized {
    const CHANNEL: &'static CStr;

    fn decode(data: &[u8]) -> Result<Self>;

    fn run(self, nelly: &mut Nelly) -> Result<Vec<u8>>;
}

trait ManagedPlatformMessage: std::fmt::Debug + Sized {
    const CHANNEL: &'static CStr;

    fn decode(reader: &mut BinaryReader<impl Read + Seek>) -> Result<Self>;

    fn run(self, nelly: &mut Nelly, writer: &mut BinaryWriter<impl Write>) -> Result<()>;
}

impl<T: ManagedPlatformMessage> RawPlatformMessage for T {
    const CHANNEL: &'static CStr = T::CHANNEL;

    fn decode(data: &[u8]) -> Result<Self> {
        T::decode(&mut BinaryReader::from(data))
    }

    fn run(self, nelly: &mut Nelly) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        self.run(nelly, &mut BinaryWriter::new(&mut bytes))?;
        Ok(bytes)
    }
}

#[derive(Debug)]
pub struct ViewIdCounter {
    serial: AtomicI64,
}

impl ViewIdCounter {
    pub const fn new() -> Self {
        Self {
            serial: AtomicI64::new(1),
        }
    }

    pub fn next_view_id(&self) -> ViewId {
        let _ = self
            .serial
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::SeqCst);
        ViewId(self.serial.fetch_add(1, Ordering::AcqRel))
    }
}

macro_rules! all_platform_message {
    (
        $(#[$meta:meta])*
        $pub:vis enum $name:ident {
            $(
                $variant:ident($ty:ty),
            )*
        }
    ) => {
        $(#[$meta])*
        $pub enum $name {
            $(
                $variant($ty),
            )*
        }

        impl $name {
            #[allow(non_upper_case_globals)]
            pub fn decode(channel: &CStr, data: &[u8]) -> Result<Self> {
                $(
                    const $variant: &[u8] = <$ty as RawPlatformMessage>::CHANNEL.to_bytes();
                )*
                match channel.to_bytes() {
                    $(
                        $variant => <$ty as RawPlatformMessage>::decode(data).map($name::$variant),
                    )*
                    _ => Err(Self::_unknown_channel(channel)),
                }
            }

            pub fn run(self, nelly: &mut Nelly) -> Result<Vec<u8>> {
                match self {
                    $(
                        $name::$variant(msg) => <$ty as RawPlatformMessage>::run(msg, nelly),
                    )*
                }
            }
        }
    };
}

all_platform_message!(
    #[derive(Debug)]
    pub enum PlatformMessage {
        CreateXdgToplevel(CreateXdgToplevel),
        UpdateXdgToplevel(UpdateXdgToplevel),
        RemoveXdgToplevel(RemoveXdgToplevel),
    }
);

impl PlatformMessage {
    fn _unknown_channel(channel: &CStr) -> std::io::Error {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "unknown platform message channel",
        )
    }
}
