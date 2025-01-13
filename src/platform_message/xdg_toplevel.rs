use std::{
    ffi::CStr,
    io::{Read, Result, Seek, Write},
};

use volito::ViewId;
use tracing::debug;

use crate::{
    embedder::FlutterWaylandSurface,
    nelly::Nelly,
    platform_message::ViewIdCounter,
    shell::{compositor::LogicalSizeConstraints, xdg::window::WindowDecorations, WaylandSurface},
};

use super::binary::{BinaryDecodable, BinaryReader, BinaryWriter};

#[derive(Debug)]
pub struct Create;

impl BinaryDecodable for Create {
    fn decode(_reader: &mut BinaryReader<impl Read + Seek>) -> Result<Self> {
        Ok(Create)
    }
}

impl super::PlatformRequest for Create {
    const CHANNEL: &'static CStr = c"wayland/xdg_toplevel/create";

    fn run(self, nelly: &mut Nelly, writer: &mut BinaryWriter<impl Write>) -> Result<()> {
        static VIEW_ID: ViewIdCounter = ViewIdCounter::new();
        let view_id = VIEW_ID.next_view_id();

        let surface = nelly.compositor_state.create_surface(&nelly.qh, view_id);

        let window =
            nelly
                .xdg_state
                .create_window(surface, WindowDecorations::ServerDefault, &nelly.qh);

        nelly
            .views
            .lock()
            .unwrap()
            .insert(view_id, FlutterWaylandSurface::from(window));

        writer.write::<i64>(&view_id.0)
    }
}

#[derive(Debug)]
pub struct InitialCommit {
    view_id: ViewId,
}

impl BinaryDecodable for InitialCommit {
    fn decode(reader: &mut BinaryReader<impl Read + Seek>) -> Result<Self> {
        Ok(Self {
            view_id: reader.read()?,
        })
    }
}

impl super::PlatformRequest for InitialCommit {
    const CHANNEL: &'static CStr = c"wayland/xdg_toplevel/initial_commit";

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
            .expect("xdg_toplevel_initial_commit: view_id not found");

        window.commit();

        Ok(())
    }
}

#[derive(Debug)]
pub struct Update {
    view_id: ViewId,
    title: String,
    app_id: String,
}

impl BinaryDecodable for Update {
    fn decode(reader: &mut BinaryReader<impl Read + Seek>) -> Result<Self> {
        Ok(Self {
            view_id: reader.read()?,
            title: reader.read_string()?,
            app_id: reader.read_string()?,
        })
    }
}

impl super::PlatformRequest for Update {
    const CHANNEL: &'static CStr = c"wayland/xdg_toplevel/update";

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

/// The view constraints are in logical pixels, as used throughout the Flutter framework (NOT the engine)
/// And this is the same as the logical pixels needed for the XDG shell protocol.
#[derive(Debug)]
pub struct UpdateViewConstraints {
    view_id: ViewId,
    constraints: LogicalSizeConstraints,
}

impl BinaryDecodable for UpdateViewConstraints {
    fn decode(reader: &mut BinaryReader<impl Read + Seek>) -> Result<Self> {
        let view_id = reader.read()?;
        let min_width = reader.read()?;
        let min_height = reader.read()?;
        let max_width = reader.read()?;
        let max_height = reader.read()?;

        reader.assert_finished().map(|()| UpdateViewConstraints {
            view_id,
            constraints: LogicalSizeConstraints {
                min_width,
                min_height,
                max_width,
                max_height,
            },
        })
    }
}

impl super::PlatformRequest for UpdateViewConstraints {
    const CHANNEL: &'static CStr = c"wayland/xdg_toplevel/update_view_constraints";

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
            .expect("update_xdg_toplevel_view_constraints: view_id not found");

        debug!("update_xdg_toplevel_view_constraints: {:?}", self);

        if let Some((min_width, min_height, max_width, max_height)) =
            self.constraints.to_wayland_constraints()
        {
            window.set_min_size(min_width, min_height);
            window.set_max_size(max_width, max_height);
        }

        let surface = window.surface().data().clone();

        drop(views);

        surface.set_logical_size_constraints(self.constraints, nelly.engine());

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
    const CHANNEL: &'static CStr = c"wayland/xdg_toplevel/remove";

    fn run(self, nelly: &mut Nelly, _writer: &mut BinaryWriter<impl Write>) -> Result<()> {
        nelly.remove_view(self.view_id)?;
        Ok(())
    }
}

pub struct Close {
    pub view_id: ViewId,
}

impl super::ManagedPlatformEvent for Close {
    const CHANNEL: &'static CStr = c"wayland/xdg_toplevel/close";

    type Response = ();

    fn encode(&self, writer: &mut BinaryWriter<impl Write>) -> Result<()> {
        writer.write::<i64>(&self.view_id.0)
    }

    fn decode_response(reader: &mut BinaryReader<impl Read + Seek>) -> Result<Self::Response> {
        reader.assert_finished()
    }
}
