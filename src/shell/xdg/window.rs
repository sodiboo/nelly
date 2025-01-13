//! XDG shell windows.

use std::{
    num::NonZeroU32,
    sync::{Arc, Mutex, Weak},
};

use smithay_client_toolkit::{
    error::GlobalError,
    globals::ProvidesBoundGlobal,
    reexports::{
        client::{
            protocol::{wl_output::WlOutput, wl_seat::WlSeat},
            Connection, Dispatch, Proxy, QueueHandle, WEnum,
        },
        csd_frame::{WindowManagerCapabilities, WindowState},
        protocols::xdg::{
            decoration::zv1::client::{
                zxdg_decoration_manager_v1,
                zxdg_toplevel_decoration_v1::{self, Mode, ZxdgToplevelDecorationV1},
            },
            shell::client::{xdg_surface, xdg_toplevel},
        },
    },
};

use crate::shell::{compositor::Surface, WaylandSurface};

use super::{XdgShell, XdgShellSurface, XdgSurface};

/// Handler for toplevel operations on a [`Window`].
pub trait WindowHandler: Sized {
    /// Request to close a window.
    ///
    /// This request does not destroy the window. You must drop all [`Window`] handles to destroy the window.
    /// This request may be sent either by the compositor or by some other mechanism (such as client side decorations).
    fn request_close(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        window: &XdgToplevelSurface,
    );

    /// Apply a suggested surface change.
    ///
    /// When this function is called, the compositor is requesting the window's size or state to change.
    ///
    /// Internally this function is called when the underlying `xdg_surface` is configured. Any extension
    /// protocols that interface with xdg-shell are able to be notified that the surface's configure sequence
    /// is complete by using this function.
    ///
    /// # Double buffering
    ///
    /// Configure events in Wayland are considered to be double buffered and the state of the window does not
    /// change until committed.
    fn configure(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        window: &XdgToplevelSurface,
        configure: WindowConfigure,
        serial: u32,
    );
}

/// Decoration mode of a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecorationMode {
    /// The window should draw client side decorations.
    Client,

    /// The server will draw window decorations.
    Server,
}

/// A window configure.
///
/// A configure describes a compositor request to resize the window or change it's state.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct WindowConfigure {
    /// The compositor suggested new size of the window in window geometry coordinates.
    ///
    /// If this value is [`None`], you may set the size of the window as you wish.
    pub new_size: (Option<NonZeroU32>, Option<NonZeroU32>),

    /// Compositor suggested maximum bounds for a window.
    ///
    /// This may be used to ensure a window is not created in a way where it will not fit.
    ///
    /// If xdg-shell is version 3 or lower, this will always be [`None`].
    pub suggested_bounds: Option<(u32, u32)>,

    /// The compositor set decoration mode of the window.
    ///
    /// This will always be [`DecorationMode::Client`] if server side decorations are not enabled or
    /// supported.
    pub decoration_mode: DecorationMode,

    /// The current state of the window.
    ///
    /// For more see [`WindowState`] documentation on the flag values.
    pub state: WindowState,

    /// The capabilities supported by the compositor.
    ///
    /// For more see [`WindowManagerCapabilities`] documentation on the flag values.
    pub capabilities: WindowManagerCapabilities,
}

impl WindowConfigure {
    /// Is [`WindowState::MAXIMIZED`] state is set.
    #[inline]
    pub fn is_maximized(&self) -> bool {
        self.state.contains(WindowState::MAXIMIZED)
    }

    /// Is [`WindowState::FULLSCREEN`] state is set.
    #[inline]
    pub fn is_fullscreen(&self) -> bool {
        self.state.contains(WindowState::FULLSCREEN)
    }

    /// Is [`WindowState::RESIZING`] state is set.
    #[inline]
    pub fn is_resizing(&self) -> bool {
        self.state.contains(WindowState::RESIZING)
    }

    /// Is [`WindowState::TILED`] state is set.
    #[inline]
    pub fn is_tiled(&self) -> bool {
        self.state.contains(WindowState::TILED)
    }

    /// Is [`WindowState::ACTIVATED`] state is set.
    #[inline]
    pub fn is_activated(&self) -> bool {
        self.state.contains(WindowState::ACTIVATED)
    }

    /// Is [`WindowState::TILED_LEFT`] state is set.
    #[inline]
    pub fn is_tiled_left(&self) -> bool {
        self.state.contains(WindowState::TILED_LEFT)
    }

    /// Is [`WindowState::TILED_RIGHT`] state is set.
    #[inline]
    pub fn is_tiled_right(&self) -> bool {
        self.state.contains(WindowState::TILED_RIGHT)
    }

    /// Is [`WindowState::TILED_TOP`] state is set.
    #[inline]
    pub fn is_tiled_top(&self) -> bool {
        self.state.contains(WindowState::TILED_TOP)
    }

    /// Is [`WindowState::TILED_BOTTOM`] state is set.
    #[inline]
    pub fn is_tiled_bottom(&self) -> bool {
        self.state.contains(WindowState::TILED_BOTTOM)
    }
}

/// Decorations a window is created with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowDecorations {
    /// The window should use the decoration mode the server asks for.
    ///
    /// The server may ask the client to render with or without client side decorations. If server side
    /// decorations are not available, client side decorations are drawn instead.
    ServerDefault,

    /// The window should request server side decorations.
    ///
    /// The server may ignore this request and ask the client to render with client side decorations. If
    /// server side decorations are not available, client side decorations are drawn instead.
    RequestServer,

    /// The window should request client side decorations.
    ///
    /// The server may ignore this request and render server side decorations. If server side decorations are
    /// not available, client side decorations are drawn.
    RequestClient,

    /// The window should always draw it's own client side decorations.
    ClientOnly,

    /// The window should use server side decorations or draw any client side decorations.
    None,
}

#[derive(Debug, Clone)]
pub struct XdgToplevelSurface(pub(super) Arc<WindowInner>);

impl XdgToplevelSurface {
    pub fn from_xdg_toplevel(toplevel: &xdg_toplevel::XdgToplevel) -> Option<XdgToplevelSurface> {
        toplevel
            .data::<WindowData>()
            .and_then(|data| data.0.upgrade())
            .map(XdgToplevelSurface)
    }

    pub fn from_xdg_surface(surface: &xdg_surface::XdgSurface) -> Option<XdgToplevelSurface> {
        surface
            .data::<WindowData>()
            .and_then(|data| data.0.upgrade())
            .map(XdgToplevelSurface)
    }

    pub fn from_toplevel_decoration(
        decoration: &ZxdgToplevelDecorationV1,
    ) -> Option<XdgToplevelSurface> {
        decoration
            .data::<WindowData>()
            .and_then(|data| data.0.upgrade())
            .map(XdgToplevelSurface)
    }

    pub fn show_window_menu(&self, seat: &WlSeat, serial: u32, position: (i32, i32)) {
        self.xdg_toplevel()
            .show_window_menu(seat, serial, position.0, position.1);
    }

    pub fn set_title(&self, title: impl Into<String>) {
        self.xdg_toplevel().set_title(title.into());
    }

    pub fn set_app_id(&self, app_id: impl Into<String>) {
        self.xdg_toplevel().set_app_id(app_id.into());
    }

    pub fn set_parent(&self, parent: Option<&XdgToplevelSurface>) {
        self.xdg_toplevel()
            .set_parent(parent.map(XdgToplevelSurface::xdg_toplevel));
    }

    pub fn set_maximized(&self) {
        self.xdg_toplevel().set_maximized()
    }

    pub fn unset_maximized(&self) {
        self.xdg_toplevel().unset_maximized()
    }

    pub fn set_minimized(&self) {
        self.xdg_toplevel().set_minimized()
    }

    pub fn set_fullscreen(&self, output: Option<&WlOutput>) {
        self.xdg_toplevel().set_fullscreen(output)
    }

    pub fn unset_fullscreen(&self) {
        self.xdg_toplevel().unset_fullscreen()
    }

    /// Requests the window should use the specified decoration mode.
    ///
    /// A mode of [`None`] indicates that the window does not care what type of decorations are used.
    ///
    /// The compositor will respond with a [`configure`](WindowHandler::configure). The configure will
    /// indicate whether the window's decoration mode has changed.
    ///
    /// # Configure loops
    ///
    /// You should avoid sending multiple decoration mode requests to ensure you do not enter a configure loop.
    pub fn request_decoration_mode(&self, mode: Option<DecorationMode>) {
        if let Some(toplevel_decoration) = &self.0.toplevel_decoration {
            match mode {
                Some(DecorationMode::Client) => toplevel_decoration.set_mode(Mode::ClientSide),
                Some(DecorationMode::Server) => toplevel_decoration.set_mode(Mode::ServerSide),
                None => toplevel_decoration.unset_mode(),
            }
        }
    }

    pub fn move_(&self, seat: &WlSeat, serial: u32) {
        self.xdg_toplevel()._move(seat, serial)
    }

    pub fn resize(&self, seat: &WlSeat, serial: u32, edges: xdg_toplevel::ResizeEdge) {
        self.xdg_toplevel().resize(seat, serial, edges)
    }

    // Double buffered window state

    pub fn set_min_size(&self, width: i32, height: i32) {
        self.xdg_toplevel().set_min_size(width, height);
    }

    /// # Protocol errors
    ///
    /// The maximum size of the window may not be smaller than the minimum size.
    pub fn set_max_size(&self, width: i32, height: i32) {
        self.xdg_toplevel().set_max_size(width, height);
    }

    // Other

    /// Returns the underlying xdg toplevel wrapped by this window.
    pub fn xdg_toplevel(&self) -> &xdg_toplevel::XdgToplevel {
        &self.0.xdg_toplevel
    }
}

impl WaylandSurface for XdgToplevelSurface {
    fn surface(&self) -> &Surface {
        self.0.xdg_surface.surface()
    }
}

impl XdgSurface for XdgToplevelSurface {
    fn xdg_surface(&self) -> &xdg_surface::XdgSurface {
        self.0.xdg_surface.xdg_surface()
    }
}

impl PartialEq for XdgToplevelSurface {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

#[derive(Debug, Clone)]
pub struct WindowData(pub(crate) Weak<WindowInner>);

#[macro_export]
macro_rules! delegate_xdg_window {
    ($(@<$( $lt:tt $( : $clt:tt $(+ $dlt:tt )* )? ),+>)? $ty: ty) => {
        ::smithay_client_toolkit::reexports::client::delegate_dispatch!($(@< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? $ty: [
            ::smithay_client_toolkit::reexports::protocols::xdg::shell::client::xdg_surface::XdgSurface: $crate::shell::xdg::window::WindowData
        ] => $crate::shell::xdg::XdgShell);
        ::smithay_client_toolkit::reexports::client::delegate_dispatch!($(@< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? $ty: [
            ::smithay_client_toolkit::reexports::protocols::xdg::shell::client::xdg_toplevel::XdgToplevel: $crate::shell::xdg::window::WindowData
        ] => $crate::shell::xdg::XdgShell);
    };
}

impl Drop for WindowInner {
    fn drop(&mut self) {
        // XDG decoration says we must destroy the decoration object before the toplevel
        if let Some(toplevel_decoration) = self.toplevel_decoration.as_ref() {
            toplevel_decoration.destroy();
        }

        // XDG Shell protocol dictates we must destroy the role object before the xdg surface.
        self.xdg_toplevel.destroy();
        // XdgShellSurface will do it's own drop
        // self.xdg_surface.destroy();
    }
}

#[derive(Debug)]
pub struct WindowInner {
    pub xdg_surface: XdgShellSurface,
    pub xdg_toplevel: xdg_toplevel::XdgToplevel,
    pub toplevel_decoration: Option<ZxdgToplevelDecorationV1>,
    pub pending_configure: Mutex<WindowConfigure>,
}

impl ProvidesBoundGlobal<zxdg_decoration_manager_v1::ZxdgDecorationManagerV1, 1> for XdgShell {
    fn bound_global(
        &self,
    ) -> Result<zxdg_decoration_manager_v1::ZxdgDecorationManagerV1, GlobalError> {
        self.xdg_decoration_manager.get().cloned()
    }
}

impl<D> Dispatch<xdg_surface::XdgSurface, WindowData, D> for XdgShell
where
    D: Dispatch<xdg_surface::XdgSurface, WindowData> + WindowHandler,
{
    fn event(
        data: &mut D,
        xdg_surface: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _: &WindowData,
        conn: &Connection,
        qh: &QueueHandle<D>,
    ) {
        if let Some(window) = XdgToplevelSurface::from_xdg_surface(xdg_surface) {
            match event {
                xdg_surface::Event::Configure { serial } => {
                    // Acknowledge the configure per protocol requirements.
                    xdg_surface.ack_configure(serial);

                    let configure = { window.0.pending_configure.lock().unwrap().clone() };
                    WindowHandler::configure(data, conn, qh, &window, configure, serial);
                }

                _ => unreachable!(),
            }
        }
    }
}

impl<D> Dispatch<xdg_toplevel::XdgToplevel, WindowData, D> for XdgShell
where
    D: Dispatch<xdg_toplevel::XdgToplevel, WindowData> + WindowHandler,
{
    fn event(
        data: &mut D,
        toplevel: &xdg_toplevel::XdgToplevel,
        event: xdg_toplevel::Event,
        _: &WindowData,
        conn: &Connection,
        qh: &QueueHandle<D>,
    ) {
        if let Some(window) = XdgToplevelSurface::from_xdg_toplevel(toplevel) {
            match event {
                xdg_toplevel::Event::Configure {
                    width,
                    height,
                    states,
                } => {
                    // The states are encoded as a bunch of u32 of native endian, but are encoded in an array of
                    // bytes.
                    let new_state = states
                        .chunks_exact(4)
                        .flat_map(TryInto::<[u8; 4]>::try_into)
                        .map(u32::from_ne_bytes)
                        .flat_map(xdg_toplevel::State::try_from)
                        .fold(WindowState::empty(), |mut acc, state| {
                            use xdg_toplevel::State;
                            match state {
                                State::Maximized => acc.set(WindowState::MAXIMIZED, true),
                                State::Fullscreen => acc.set(WindowState::FULLSCREEN, true),
                                State::Resizing => acc.set(WindowState::RESIZING, true),
                                State::Activated => acc.set(WindowState::ACTIVATED, true),
                                State::TiledLeft => acc.set(WindowState::TILED_LEFT, true),
                                State::TiledRight => acc.set(WindowState::TILED_RIGHT, true),
                                State::TiledTop => acc.set(WindowState::TILED_TOP, true),
                                State::TiledBottom => acc.set(WindowState::TILED_BOTTOM, true),
                                State::Suspended => acc.set(WindowState::SUSPENDED, true),
                                _ => (),
                            }
                            acc
                        });

                    // XXX we do explicit convertion and sanity checking because compositor
                    // could pass negative values which we should ignore all together.
                    let width = u32::try_from(width).ok().and_then(NonZeroU32::new);
                    let height = u32::try_from(height).ok().and_then(NonZeroU32::new);

                    let pending_configure = &mut window.0.pending_configure.lock().unwrap();
                    pending_configure.new_size = (width, height);
                    pending_configure.state = new_state;
                }

                xdg_toplevel::Event::Close => {
                    data.request_close(conn, qh, &window);
                }

                xdg_toplevel::Event::ConfigureBounds { width, height } => {
                    let pending_configure = &mut window.0.pending_configure.lock().unwrap();
                    if width == 0 && height == 0 {
                        pending_configure.suggested_bounds = None;
                    } else {
                        pending_configure.suggested_bounds = Some((width as u32, height as u32));
                    }
                }
                xdg_toplevel::Event::WmCapabilities { capabilities } => {
                    let pending_configure = &mut window.0.pending_configure.lock().unwrap();
                    pending_configure.capabilities = capabilities
                        .chunks_exact(4)
                        .flat_map(TryInto::<[u8; 4]>::try_into)
                        .map(u32::from_ne_bytes)
                        .flat_map(xdg_toplevel::WmCapabilities::try_from)
                        .fold(WindowManagerCapabilities::empty(), |mut acc, capability| {
                            use xdg_toplevel::WmCapabilities;
                            match capability {
                                WmCapabilities::WindowMenu => {
                                    acc.set(WindowManagerCapabilities::WINDOW_MENU, true)
                                }
                                WmCapabilities::Maximize => {
                                    acc.set(WindowManagerCapabilities::MAXIMIZE, true)
                                }
                                WmCapabilities::Fullscreen => {
                                    acc.set(WindowManagerCapabilities::FULLSCREEN, true)
                                }
                                WmCapabilities::Minimize => {
                                    acc.set(WindowManagerCapabilities::MINIMIZE, true)
                                }
                                _ => (),
                            }
                            acc
                        });
                }
                _ => unreachable!(),
            }
        }
    }
}

// XDG decoration

impl<D> Dispatch<zxdg_decoration_manager_v1::ZxdgDecorationManagerV1, (), D> for XdgShell
where
    D: Dispatch<zxdg_decoration_manager_v1::ZxdgDecorationManagerV1, ()> + WindowHandler,
{
    fn event(
        _: &mut D,
        _: &zxdg_decoration_manager_v1::ZxdgDecorationManagerV1,
        _: zxdg_decoration_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<D>,
    ) {
        unreachable!("zxdg_decoration_manager_v1 has no events")
    }
}

impl<D> Dispatch<ZxdgToplevelDecorationV1, WindowData, D> for XdgShell
where
    D: Dispatch<ZxdgToplevelDecorationV1, WindowData> + WindowHandler,
{
    fn event(
        _: &mut D,
        decoration: &ZxdgToplevelDecorationV1,
        event: zxdg_toplevel_decoration_v1::Event,
        _: &WindowData,
        _: &Connection,
        _: &QueueHandle<D>,
    ) {
        if let Some(window) = XdgToplevelSurface::from_toplevel_decoration(decoration) {
            match event {
                zxdg_toplevel_decoration_v1::Event::Configure { mode } => match mode {
                    WEnum::Value(mode) => {
                        let mode = match mode {
                            Mode::ClientSide => DecorationMode::Client,
                            Mode::ServerSide => DecorationMode::Server,

                            _ => unreachable!(),
                        };

                        window.0.pending_configure.lock().unwrap().decoration_mode = mode;
                    }

                    WEnum::Unknown(unknown) => {
                        tracing::error!(target: "sctk", "unknown decoration mode 0x{:x}", unknown);
                    }
                },

                _ => unreachable!(),
            }
        }
    }
}
