use std::{
    ffi::CStr,
    io::{Read, Result, Seek, Write},
};

use halcyon_embedder::platform_message::{
    binary::{BinaryDecodable, BinaryReader, BinaryWriter},
    ManagedPlatformRequest, PlatformMessageChannel,
};

use crate::Nelly;

#[derive(Debug)]
pub struct Shutdown;

impl BinaryDecodable for Shutdown {
    fn decode(_reader: &mut BinaryReader<impl Read + Seek>) -> Result<Self> {
        Ok(Shutdown)
    }
}

impl PlatformMessageChannel for Shutdown {
    const CHANNEL: &'static CStr = c"nelly/graceful_shutdown";
}

impl ManagedPlatformRequest<Nelly> for Shutdown {
    fn run(self, nelly: &mut Nelly, _writer: &mut BinaryWriter<impl Write>) -> Result<()> {
        nelly.loop_signal.stop();
        Ok(())
    }
}
