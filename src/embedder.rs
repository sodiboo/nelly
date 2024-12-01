mod gen {
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

use fluster::{
    AOTData, AOTDataSource, BackingStore, BackingStoreConfig, Engine, Layer, LayerContent,
    ProjectArgs, SoftwareBackingStore, SoftwarePixelFormat, SoftwareRendererConfig, ViewId,
};
use gen::{APP_LIBRARY, ASSETS};
use smithay_client_toolkit::{
    compositor::{Surface, SurfaceData, SurfaceDataExt},
    reexports::{
        calloop::{
            self,
            channel::{channel, Sender},
            LoopHandle,
        },
        client::{
            delegate_noop,
            protocol::{wl_buffer, wl_shm, wl_shm_pool, wl_surface::WlSurface},
            QueueHandle,
        },
        protocols::xdg::shell::client::xdg_toplevel::XdgToplevel,
    },
    shell::{xdg::window::Window, WaylandSurface},
    shm::Shm,
};
use tracing::{debug, error, trace, warn};

use crate::{config::Config, nelly::Nelly, pool::SinglePool};

enum EmbedderMessage {
    Vsync(fluster::VsyncBaton),
}

pub struct Handler {
    config: Rc<RefCell<Config>>,
    msg: Sender<EmbedderMessage>,
}

impl fluster::SoftwareRendererHandler for Handler {
    fn surface_present(&mut self, allocation: *const u8, row_bytes: usize, height: usize) -> bool {
        debug!("surface present");
        true
    }
}

impl fluster::EngineHandler for Handler {
    fn platform_message(
        &mut self,
        channel: &std::ffi::CStr,
        message: &[u8],
        response: fluster::PlatformMessageResponse,
    ) {
        println!("platform message: {:?}", message);
        response.send(&[]).unwrap(); // send empty response to avoid memory leak
    }

    fn vsync(&mut self, baton: fluster::VsyncBaton) {
        self.msg.send(EmbedderMessage::Vsync(baton)).unwrap();
    }

    fn update_semantics(&mut self, update: fluster::SemanticsUpdate) {
        println!("update semantics");
    }

    fn log_message(&mut self, tag: &std::ffi::CStr, message: &std::ffi::CStr) {
        println!("log message: [{tag:?}] {}", message.to_str().unwrap());
    }

    fn on_pre_engine_restart(&mut self) {
        println!("pre engine restart");
    }

    fn channel_update(&mut self, channel: &std::ffi::CStr, listening: bool) {
        println!("channel update: {channel:?}, {listening}");
    }

    fn root_isolate_created(&mut self) {
        println!("root isolate created");
    }
}

macro_rules! pixfmt {
    (
        $(
            $variant:ident => wl_shm::$wl_shm:ident, flutter::$flutter:ident;
        )*
    ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        enum PixelFormat {
            $($variant,)*
        }

        impl From<PixelFormat> for wl_shm::Format {
            fn from(value: PixelFormat) -> Self {
                match value {
                    $(PixelFormat::$variant => wl_shm::Format::$wl_shm,)*
                }
            }
        }

        impl From<PixelFormat> for fluster::SoftwarePixelFormat {
            fn from(value: PixelFormat) -> Self {
                match value {
                    $(PixelFormat::$variant => fluster::SoftwarePixelFormat::$flutter,)*
                }
            }
        }

        impl TryFrom<wl_shm::Format> for PixelFormat {
            type Error = wl_shm::Format;

            fn try_from(value: wl_shm::Format) -> Result<Self, Self::Error> {
                match value {
                    $(wl_shm::Format::$wl_shm => Ok(PixelFormat::$variant),)*
                    v => Err(v),
                }
            }
        }

    };
}

pixfmt! {
    Rgba8888 => wl_shm::Rgba8888, flutter::RGBA8888;
    Bgra8888 => wl_shm::Bgra8888, flutter::BGRA8888;
    Rgba4444 => wl_shm::Rgba4444, flutter::RGBA4444;
    Rgbx8888 => wl_shm::Rgbx8888, flutter::RGBX8888;
    Rgb565 => wl_shm::Rgb565, flutter::RGB565;

    Split => wl_shm::Argb8888, flutter::RGBA8888;
}

impl PixelFormat {
    fn bytes(self) -> usize {
        match self {
            PixelFormat::Rgb565 => 2,
            PixelFormat::Rgba4444 => 2,
            PixelFormat::Rgba8888 => 4,
            PixelFormat::Rgbx8888 => 4,
            PixelFormat::Bgra8888 => 4,

            PixelFormat::Split => 4, // sizeof(Argb8888) == sizeof(Rgba8888)
        }
    }
}

struct NellyCompositor {
    config: Rc<RefCell<Config>>,
    msg: Sender<EmbedderMessage>,

    qh: QueueHandle<Nelly>,
    wl_shm: wl_shm::WlShm,

    views: HashMap<ViewId, FlutterWaylandSurface>,

    format: PixelFormat,
}

pub enum FlutterWaylandSurface {
    XdgToplevel(Window),
}

impl FlutterWaylandSurface {
    fn surface(&self) -> &WlSurface {
        match self {
            FlutterWaylandSurface::XdgToplevel(toplevel) => toplevel.wl_surface(),
        }
    }
}

impl From<Window> for FlutterWaylandSurface {
    fn from(window: Window) -> Self {
        FlutterWaylandSurface::XdgToplevel(window)
    }
}

impl fluster::CompositorHandler for NellyCompositor {
    fn create_backing_store(&mut self, config: BackingStoreConfig) -> Option<BackingStore> {
        let width = config.size.width as usize;
        let height = config.size.height as usize;
        debug!(
            "backing store: {} -> {}",
            format_args!("{}x{}", config.size.width, config.size.height),
            format_args!("{width}x{height}")
        );

        let row_bytes = width * self.format.bytes();

        let layout = std::alloc::Layout::from_size_align(row_bytes * height, 1)
            .inspect_err(|e| {
                error!("Failed to allocate backing store: {:?}", e);
            })
            .ok()?;

        let allocation = unsafe { std::alloc::alloc(layout) };

        Some(BackingStore::Software(SoftwareBackingStore {
            allocation,
            row_bytes,
            height,
            pixel_format: self.format.into(),
        }))
    }

    fn collect_backing_store(&mut self, backing_store: BackingStore) -> bool {
        match backing_store {
            BackingStore::Software(SoftwareBackingStore {
                allocation,
                row_bytes,
                height,
                pixel_format: _,
            }) => {
                let layout = std::alloc::Layout::from_size_align(row_bytes * height, 1).unwrap();

                unsafe { std::alloc::dealloc(allocation, layout) };
                true
            }
            _ => {
                error!("collect backing store but not software");
                false
            }
        }
    }

    fn present_view(&mut self, view_id: ViewId, layers: &[fluster::Layer]) -> bool {
        let Some(view) = self.views.get(&view_id) else {
            error!("flutter gave me a view id i don't know about");
            return false;
        };

        let FlutterWaylandSurface::XdgToplevel(window) = view;
        let surface = window.wl_surface();

        let [layer] = layers else {
            error!(
                "flutter gave me {} layers, but i can't composite any other amount than one",
                layers.len()
            );
            return false;
        };

        let LayerContent::BackingStore(backing_store, present_info) = &layer.content else {
            error!("flutter gave me a layer with a platform view");
            return false;
        };

        let &BackingStore::Software(SoftwareBackingStore {
            allocation,
            row_bytes,
            height,
            pixel_format,
        }) = backing_store
        else {
            error!("flutter gave me a backing store i can't handle (i only ever submitted software backing stores)");
            return false;
        };

        if pixel_format != self.format.into() {
            error!("flutter gave me a backing store with a pixel format i didn't expect");
            return false;
        }

        let width = (row_bytes / self.format.bytes()) as i32;
        let height = height as i32;
        let stride = row_bytes as i32;

        let pool = match SinglePool::new(
            width,
            height,
            stride,
            self.format.into(),
            &self.qh,
            &self.wl_shm,
        ) {
            Ok(pool) => pool,
            Err(e) => {
                error!("failed to create a pool: {:?}", e);
                return false;
            }
        };

        surface.attach(Some(pool.buffer()), 0, 0);

        for rect in present_info.paint_region.regions.iter() {
            debug!("painting rect: {:?}", rect);
            let (x, y) = (rect.left, rect.top);
            let (width, height) = (rect.right, rect.bottom);

            if x.fract() != 0.0 || y.fract() != 0.0 || width.fract() != 0.0 || height.fract() != 0.0
            {
                error!("paint region {rect:?} is not pixel aligned");
                return false;
            }

            let (x, y) = (x as i32, y as i32);
            let (width, height) = (width as i32, height as i32);

            surface.damage_buffer(x, y, width, height);

            for i in y..height {
                let src = allocation;
                let src = unsafe {
                    src.byte_offset((i * stride) as isize)
                        .byte_offset((x * self.format.bytes() as i32) as isize)
                };
                let dst = pool.mmap().as_mut_ptr();
                let dst = unsafe {
                    dst.offset((i * stride) as isize)
                        .offset((x * self.format.bytes() as i32) as isize)
                };

                if self.format == PixelFormat::Split {
                    // src is Rgba8888, dst is Argb8888
                    let src = src as *const [u8; 4];
                    let dst = dst as *mut [u8; 4];
                    for j in 0..width as isize {
                        let [r, g, b, a] = unsafe { std::ptr::read(src.offset(j)) };
                        unsafe { std::ptr::write(dst.offset(j), [a, r, g, b]) };
                    }
                } else {
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            &raw const *src,
                            dst,
                            width as usize * self.format.bytes(),
                        );
                    }
                }
            }
        }
        surface.commit();

        debug!("presented view {view_id:?}");

        true
    }
}

pub fn init(
    config: Rc<RefCell<Config>>,
    loop_handle: LoopHandle<Nelly>,
    shm: &Shm,
    qh: QueueHandle<Nelly>,
    implicit_surface: FlutterWaylandSurface,
) -> anyhow::Result<Engine> {
    debug!("{:?}", shm.formats());
    let supported_formats: HashSet<PixelFormat> = shm
        .formats()
        .iter()
        .copied()
        .map(PixelFormat::try_from)
        .filter_map(Result::ok)
        .collect();

    debug!("Supported formats: {:?}", supported_formats);

    let format = [
        PixelFormat::Rgba8888,
        PixelFormat::Bgra8888,
        PixelFormat::Rgba4444,
        PixelFormat::Rgbx8888,
        PixelFormat::Rgb565,
        PixelFormat::Split,
    ]
    .iter()
    .find(|format| supported_formats.contains(format))
    .copied();

    let Some(format) = format else {
        anyhow::bail!("The wayland compositor doesn't support the required Argb8888 wl_shm format. \
            I'd much rather it support Rgba8888 or Bgra8888, since those can work directly with the Flutter engine. \
            But as a fallback, i can also convert to Argb8888, which is required by the core Wayland protocol.");
    };
    trace!("Using {:?} as the pixel format.", format);
    match format {
        PixelFormat::Rgba8888 | PixelFormat::Bgra8888 => {
            // These formats are optimal for the Flutter engine.
        }
        PixelFormat::Rgba4444 => {
            warn!("Compositor doesn't support Rgba8888 or Bgra8888, which are optimal for the Flutter engine. \
                Falling back to Rgba4444, which has lower bit depth. \
                Colors will be less accurate.");
        }
        PixelFormat::Rgbx8888 => {
            warn!("Compositor doesn't support Rgba8888 or Bgra8888, which are optimal for the Flutter engine. \
                Falling back to Rgbx8888, which doesn't support transparency. \
                All surfaces will be opaque.");
        }
        PixelFormat::Rgb565 => {
            warn!("Compositor doesn't support Rgba8888 or Bgra8888, which are optimal for the Flutter engine. \
                Falling back to Rgb565, which has lower bit depth and doesn't support transparency. \
                Colors will be less accurate and all surfaces will be opaque.");
        }

        PixelFormat::Split => {
            warn!("Compositor doesn't support any pixel format that Flutter supports. \
                Falling back to a rendering as Rgba8888 and submitting Argb8888. \
                This will require a conversion step, which may be slower, especially on high resolution displays. \
                Consider implementing Rgba8888 or Bgra8888 support in the compositor.");
        }
    }

    let aot_data = APP_LIBRARY
        .map(PathBuf::from)
        .map(AOTDataSource::ElfPath)
        .as_ref()
        .map(AOTData::new)
        .transpose()
        .unwrap()
        // It's okay to share the AOT data across several engines.
        // We don't do that, but we still need to wrap it in an Arc.
        .map(Arc::new);

    if AOTData::is_aot() {
        assert!(
            aot_data.is_some(),
            "AOT data is required, but was not built"
        );
    } else {
        assert!(aot_data.is_none(), "AOT data is not allowed, but was built");
    }

    let (send, chan) = channel();

    loop_handle
        .insert_source(chan, |msg, _, nelly| {
            match msg {
                calloop::channel::Event::Msg(msg) => {
                    match msg {
                        EmbedderMessage::Vsync(vsync_baton) => {
                            debug!("vsync: {:?}", vsync_baton);
                            nelly
                                .engine()
                                .on_vsync(
                                    vsync_baton,
                                    Engine::get_current_time(),
                                    Engine::get_current_time(),
                                )
                                .unwrap();
                        }
                    };
                }
                calloop::channel::Event::Closed => {
                    debug!("embedder channel closed");
                }
            };
        })
        .unwrap();

    let engine = Engine::run(
        SoftwareRendererConfig {
            handler: Box::new(Handler {
                config: config.clone(),
                msg: send.clone(),
            }),
        },
        ProjectArgs {
            assets_path: Path::new(ASSETS),
            icu_data_path: Path::new(fluster::build::ICU_DATA),
            command_line_argv: &[],
            persistent_cache_path: None,
            is_persistent_cache_read_only: true,
            custom_dart_entrypoint: None,
            custom_task_runners: None,
            shutdown_dart_vm_when_done: true,
            compositor: Some(fluster::Compositor {
                avoid_backing_store_cache: true,
                handler: Box::new(NellyCompositor {
                    config: config.clone(),
                    msg: send.clone(),
                    qh,
                    wl_shm: shm.wl_shm().clone(),
                    // buffers: HashMap::new(),
                    views: HashMap::from([(ViewId::IMPLICIT, implicit_surface)]),
                    format,
                }),
            }),
            dart_entrypoint_argv: &["hello", "world", "from", "Rust"],
            log_tag: c"nelly".into(),
            dart_old_gen_heap_size: 0,
            aot_data,
            handler: Box::new(Handler {
                config: config.clone(),
                msg: send.clone(),
            }),
            compute_platform_resolved_locale: None,
        },
    )?;

    Ok(engine)
}
