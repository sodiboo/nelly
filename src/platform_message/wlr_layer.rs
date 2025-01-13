use std::{
    ffi::CStr,
    io::{Read, Result, Seek, Write},
};

use volito::ViewId;
use tracing::{error, info};

use crate::{
    embedder::FlutterWaylandSurface,
    nelly::{Nelly, NellyEvent},
    platform_message::ViewIdCounter,
    shell::layer::{Anchor, Layer},
};

use super::binary::{BinaryDecodable, BinaryReader, BinaryWriter};

impl BinaryDecodable for Layer {
    fn decode(reader: &mut BinaryReader<impl Read + Seek>) -> Result<Self> {
        match reader.read::<u8>()? {
            0 => Ok(Self::Background),
            1 => Ok(Self::Bottom),
            2 => Ok(Self::Top),
            3 => Ok(Self::Overlay),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid layer value",
            )),
        }
    }
}

impl BinaryDecodable for Anchor {
    fn decode(reader: &mut BinaryReader<impl Read + Seek>) -> Result<Self> {
        Anchor::from_bits(reader.read::<u32>()?).ok_or(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid anchor value",
        ))
    }
}

#[derive(Debug)]
pub struct Create {
    layer: Layer,
    namespace: String,
}

impl BinaryDecodable for Create {
    fn decode(reader: &mut BinaryReader<impl Read + Seek>) -> Result<Self> {
        Ok(Self {
            layer: reader.read()?,
            namespace: reader.read_string()?,
        })
    }
}

impl super::PlatformRequest for Create {
    const CHANNEL: &'static CStr = c"wayland/wlr_layer/create";

    fn run(self, nelly: &mut Nelly, writer: &mut BinaryWriter<impl Write>) -> Result<()> {
        static VIEW_ID: ViewIdCounter = ViewIdCounter::new();
        let view_id = VIEW_ID.next_view_id();

        let surface = nelly.compositor_state.create_surface(&nelly.qh, view_id);

        let layer = nelly.layer_shell.create_layer_surface(
            &nelly.qh,
            surface,
            self.layer,
            self.namespace,
            None,
        );

        nelly
            .views
            .lock()
            .unwrap()
            .insert(view_id, FlutterWaylandSurface::from(layer));

        writer.write::<i64>(&view_id.0)
    }
}

#[derive(Debug)]
pub struct Update {
    view_id: ViewId,

    width: u32,
    height: u32,

    anchor: Anchor,
}

impl BinaryDecodable for Update {
    fn decode(reader: &mut BinaryReader<impl Read + Seek>) -> Result<Self> {
        Ok(Self {
            view_id: reader.read()?,
            width: reader.read()?,
            height: reader.read()?,
            anchor: reader.read()?,
        })
    }
}

impl super::PlatformRequest for Update {
    const CHANNEL: &'static CStr = c"wayland/wlr_layer/update";

    fn run(self, nelly: &mut Nelly, _writer: &mut BinaryWriter<impl Write>) -> Result<()> {
        let views = nelly.views.lock().unwrap();
        let window = views
            .get(&self.view_id)
            .and_then(|surface| {
                if let FlutterWaylandSurface::WlrLayer(surface) = surface {
                    Some(surface)
                } else {
                    None
                }
            })
            .expect("wlr_layer/update: view_id not found");

        window.set_size(self.width, self.height);
        window.set_anchor(self.anchor);

        Ok(())
    }
}

#[derive(Debug)]
pub struct Remove {
    view_id: ViewId,
}

impl BinaryDecodable for Remove {
    fn decode(reader: &mut BinaryReader<impl Read + Seek>) -> Result<Self> {
        Ok(Self {
            view_id: reader.read()?,
        })
    }
}

impl super::PlatformRequest for Remove {
    const CHANNEL: &'static CStr = c"wayland/wlr_layer/remove";

    fn run(self, nelly: &mut Nelly, _writer: &mut BinaryWriter<impl Write>) -> Result<()> {
        nelly.remove_view(self.view_id)?;

        Ok(())
    }
}

// pub struct CloseXdgToplevel {
//     pub view_id: ViewId,
// }

// impl super::ManagedPlatformEvent for CloseXdgToplevel {
//     const CHANNEL: &'static CStr = c"wayland/wlr_layer/close";

//     type Response = ();

//     fn encode(&self, writer: &mut BinaryWriter<impl Write>) -> Result<()> {
//         writer.write::<i64>(&self.view_id.0)
//     }

//     fn decode_response(reader: &mut BinaryReader<impl Read + Seek>) -> Result<Self::Response> {
//         reader.assert_finished()
//     }
// }
