use std::{
    ffi::CStr,
    io::{Read, Result, Seek, Write},
    sync::atomic::{AtomicI64, Ordering},
};

use binary::BinaryDecodable;
use volito::ViewId;

use crate::nelly::Nelly;

mod binary;
pub mod shutdown;
pub mod wlr_layer;
pub mod xdg_toplevel;

use self::binary::{BinaryReader, BinaryWriter};

trait PlatformRequest: std::fmt::Debug + BinaryDecodable {
    const CHANNEL: &'static CStr;

    fn run(self, nelly: &mut Nelly, writer: &mut BinaryWriter<impl Write>) -> Result<()>;
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
            pub fn _decode(channel: &CStr, reader: &mut BinaryReader<impl Read + Seek>) -> Result<Self> {
                $(
                    const $variant: &[u8] = <$ty as PlatformRequest>::CHANNEL.to_bytes();
                )*
                match channel.to_bytes() {
                    $(
                        $variant => <$ty as BinaryDecodable>::decode(reader).map($name::$variant),
                    )*
                    _ => Err(Self::_unknown_channel(channel)),
                }
            }

            pub fn _run(self, nelly: &mut Nelly, writer: &mut BinaryWriter<impl Write>) -> Result<()> {
                match self {
                    $(
                        $name::$variant(msg) => <$ty as PlatformRequest>::run(msg, nelly, writer),
                    )*
                }
            }
        }
    };
}

all_platform_message!(
    #[derive(Debug)]
    pub enum AnyPlatformRequest {
        Shutdown(shutdown::Shutdown),

        XdgToplevelCreate(xdg_toplevel::Create),
        XdgToplevelInitialCommit(xdg_toplevel::InitialCommit),
        XdgToplevelUpdate(xdg_toplevel::Update),
        XdgToplevelUpdateViewConstraints(xdg_toplevel::UpdateViewConstraints),
        XdgToplevelRemove(xdg_toplevel::Remove),

        WlrLayerCreate(wlr_layer::Create),
        WlrLayerUpdate(wlr_layer::Update),
        WlrLayerRemove(wlr_layer::Remove),
    }
);

impl AnyPlatformRequest {
    fn _unknown_channel(channel: &CStr) -> std::io::Error {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "unknown platform message channel: {}",
                channel.to_string_lossy()
            ),
        )
    }

    pub fn decode(channel: &CStr, data: &[u8]) -> Result<Self> {
        let mut reader = BinaryReader::from(data);

        let decoded = Self::_decode(channel, &mut reader)?;

        reader.assert_finished()?;

        Ok(decoded)
    }

    pub fn run(self, nelly: &mut Nelly) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();

        self._run(nelly, &mut BinaryWriter::new(&mut bytes))?;

        Ok(bytes)
    }
}

pub trait PlatformEvent {
    const CHANNEL: &'static CStr;

    type Response: 'static;

    fn encode(&self) -> Result<Vec<u8>>;

    fn decode_response(data: &[u8]) -> Result<Self::Response>;

    fn send(
        &self,
        nelly: &mut Nelly,
        f: impl FnOnce(Result<Self::Response>, &mut Nelly) + 'static,
    ) -> Result<()> {
        let data = self.encode()?;

        let loop_handle = nelly.loop_handle.clone();
        let loop_signal = nelly.loop_signal.clone();

        nelly
            .engine()
            .send_platform_message(Self::CHANNEL, data.as_slice(), move |response| {
                let response = Self::decode_response(response);
                loop_handle.insert_idle(|nelly| f(response, nelly));
                loop_signal.wakeup();
            })
            .map_err(Into::into)
    }
}

trait ManagedPlatformEvent {
    const CHANNEL: &'static CStr;

    type Response: 'static;

    fn encode(&self, writer: &mut BinaryWriter<impl Write>) -> Result<()>;

    fn decode_response(reader: &mut BinaryReader<impl Read + Seek>) -> Result<Self::Response>;
}

impl<R: 'static, T: ManagedPlatformEvent<Response = R>> PlatformEvent for T {
    const CHANNEL: &'static CStr = Self::CHANNEL;

    type Response = R;

    fn encode(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        self.encode(&mut BinaryWriter::new(&mut bytes))?;
        Ok(bytes)
    }

    fn decode_response(data: &[u8]) -> Result<Self::Response> {
        T::decode_response(&mut BinaryReader::from(data))
    }
}
