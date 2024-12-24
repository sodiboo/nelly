use std::{
    ffi::CStr,
    io::{Read, Result, Seek, Write},
};

use fluster::ViewId;
use tracing::{error, info};

use crate::{
    embedder::FlutterWaylandSurface,
    nelly::{Nelly, NellyEvent},
    platform_message::ViewIdCounter,
    shell::{xdg::window::WindowDecorations, WaylandSurface},
};

use super::binary::{BinaryReader, BinaryWriter};

#[derive(Debug)]
pub struct CreateXdgToplevel {
    title: String,
    app_id: String,
}
impl super::ManagedPlatformMessage for CreateXdgToplevel {
    const CHANNEL: &'static CStr = c"nelly/create_xdg_toplevel";

    fn decode(reader: &mut BinaryReader<impl Read + Seek>) -> Result<Self> {
        let title = reader.read_string()?;
        let app_id = reader.read_string()?;

        Ok(CreateXdgToplevel { title, app_id })
    }

    fn run(self, nelly: &mut Nelly, writer: &mut BinaryWriter<impl Write>) -> Result<()> {
        static VIEW_ID: ViewIdCounter = ViewIdCounter::new();
        let view_id = VIEW_ID.next_view_id();
        info!("Creating xdg_toplevel with title: {}", self.title);
        info!("will receive view_id: {:?}", view_id);

        let surface = nelly.compositor_state.create_surface(&nelly.qh, view_id);

        let window =
            nelly
                .xdg_state
                .create_window(surface, WindowDecorations::ServerDefault, &nelly.qh);

        window.set_title(self.title);
        window.set_app_id(self.app_id);
        window.commit();

        nelly
            .views
            .lock()
            .unwrap()
            .insert(view_id, FlutterWaylandSurface::from(window));

        writer.write::<i64>(&view_id.0)
    }
}

#[derive(Debug)]
pub struct UpdateXdgToplevel {
    view_id: ViewId,
    title: String,
    app_id: String,
}

impl super::ManagedPlatformMessage for UpdateXdgToplevel {
    const CHANNEL: &'static CStr = c"nelly/update_xdg_toplevel";

    fn decode(reader: &mut BinaryReader<impl Read + Seek>) -> Result<Self> {
        let view_id = reader.read::<i64>()?;
        let title = reader.read_string()?;
        let app_id = reader.read_string()?;

        Ok(UpdateXdgToplevel {
            view_id: ViewId(view_id),
            title,
            app_id,
        })
    }

    fn run(self, nelly: &mut Nelly, _writer: &mut BinaryWriter<impl Write>) -> Result<()> {
        let views = nelly.views.lock().unwrap();
        let window = views
            .get(&self.view_id)
            .and_then(|surface| {
                if let FlutterWaylandSurface::XdgToplevel(surface) = surface {
                    Some(surface)
                } else {
                    None
                }
            })
            .expect("update_xdg_toplevel: view_id not found");

        window.set_title(&self.title);
        window.set_app_id(&self.app_id);

        Ok(())
    }
}

#[derive(Debug)]
pub struct RemoveXdgToplevel {
    view_id: ViewId,
}

impl super::ManagedPlatformMessage for RemoveXdgToplevel {
    const CHANNEL: &'static CStr = c"nelly/remove_xdg_toplevel";

    fn decode(reader: &mut BinaryReader<impl Read + Seek>) -> Result<Self> {
        let view_id = reader.read::<i64>()?;

        Ok(RemoveXdgToplevel {
            view_id: ViewId(view_id),
        })
    }

    fn run(self, nelly: &mut Nelly, _writer: &mut BinaryWriter<impl Write>) -> Result<()> {
        nelly.views.lock().unwrap().remove(&self.view_id);
        let events = nelly.events().clone();
        nelly.engine().remove_view(self.view_id, move |success| {
            if success {
                events
                    .send(NellyEvent::ViewRemoved(self.view_id))
                    .expect("Nelly event channel closed");
            } else {
                error!("Failed to remove xdg_toplevel view {:?}", self.view_id);
            }
        })?;

        Ok(())
    }
}
