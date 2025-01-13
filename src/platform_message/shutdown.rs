use std::{
    ffi::CStr,
    io::{Read, Result, Seek, Write},
};

use crate::nelly::Nelly;

use super::binary::{BinaryDecodable, BinaryReader, BinaryWriter};

#[derive(Debug)]
pub struct Shutdown;

impl BinaryDecodable for Shutdown {
    fn decode(_reader: &mut BinaryReader<impl Read + Seek>) -> Result<Self> {
        Ok(Shutdown)
    }
}

impl super::PlatformRequest for Shutdown {
    const CHANNEL: &'static CStr = c"nelly/graceful_shutdown";

    fn run(self, nelly: &mut Nelly, _writer: &mut BinaryWriter<impl Write>) -> Result<()> {
        nelly.loop_signal.stop();
        Ok(())
    }
}
