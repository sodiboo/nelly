use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    marker::PhantomData,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{atomic::Ordering, Arc, Mutex},
};

use fluster::{
    AOTData, AOTDataSource, BackingStore, BackingStoreConfig, Engine, Layer, LayerContent,
    ProjectArgs, SoftwareBackingStore, SoftwarePixelFormat, SoftwareRendererConfig, ViewId,
    WindowMetricsEvent,
};
use smithay_client_toolkit::{
    reexports::{
        calloop::{
            self,
            channel::{channel, Sender},
            LoopHandle,
        },
        client::{
            delegate_noop,
            protocol::{
                wl_buffer::{self, WlBuffer},
                wl_shm, wl_shm_pool,
                wl_surface::WlSurface,
            },
            Proxy, QueueHandle,
        },
    },
    session_lock::SessionLockSurface,
    shell::wlr_layer::LayerSurface,
    shm::Shm,
};
use tracing::{debug, error, info, trace, warn};

use crate::{
    config::Config,
    nelly::Nelly,
    platform_message::PlatformMessage,
    pool::SinglePool,
    shell::{
        compositor::{Surface, SurfaceData},
        xdg::{popup::Popup, window::Window},
        WaylandSurface,
    },
};

enum EmbedderMessage {
    Ping,
    Vsync(fluster::VsyncBaton),
    PlatformMessage(PlatformMessage, fluster::PlatformMessageResponse),
}

pub struct Handler {
    config: Arc<Mutex<Config>>,
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
        info!("received platform message: {channel:?}, {message:?}");
        match PlatformMessage::decode(channel, message) {
            Ok(message) => {
                info!("decoded platform message: {message:?}");
                self.msg
                    .send(EmbedderMessage::PlatformMessage(message, response))
                    .unwrap()
            }
            Err(e) => {
                error!("{e:?}");
                response.send(&[]).unwrap(); // send empty response, avoids memory leak
            }
        }
    }

    fn vsync(&mut self, baton: fluster::VsyncBaton) {
        self.msg.send(EmbedderMessage::Vsync(baton)).unwrap();
    }

    fn update_semantics(&mut self, update: fluster::SemanticsUpdate) {
        debug!("update semantics");
    }

    fn log_message(&mut self, tag: &std::ffi::CStr, message: &std::ffi::CStr) {
        let tag = tag.to_string_lossy();
        let message = message.to_string_lossy();
        log::info!(target: &tag, "{message}");
    }

    fn on_pre_engine_restart(&mut self) {
        debug!("pre engine restart");
    }

    fn channel_update(&mut self, channel: &std::ffi::CStr, listening: bool) {
        debug!("channel update: {channel:?}, {listening}");
    }

    fn root_isolate_created(&mut self) {
        // crate::ffi::init_resolver();
        debug!("root isolate created");
    }
}

/// The singular pixel format used by the software renderer.
struct PixelFormat;

impl From<PixelFormat> for wl_shm::Format {
    fn from(PixelFormat: PixelFormat) -> Self {
        wl_shm::Format::Argb8888 // u32 (little-endian)
    }
}

impl From<PixelFormat> for fluster::SoftwarePixelFormat {
    fn from(PixelFormat: PixelFormat) -> Self {
        fluster::SoftwarePixelFormat::BGRA8888 // [u8; 4]
    }
}

impl PixelFormat {
    #[allow(clippy::unused_self)]
    const fn bytes(self) -> usize {
        4
    }
}

#[derive(Copy, Clone, Hash, PartialEq, Eq)]
struct BackingStoreAllocation(*mut u8);

unsafe impl Send for BackingStoreAllocation {}
unsafe impl Sync for BackingStoreAllocation {}

struct NellyCompositor {
    config: Arc<Mutex<Config>>,
    msg: Sender<EmbedderMessage>,

    qh: QueueHandle<Nelly>,
    wl_shm: wl_shm::WlShm,

    buffers: HashMap<BackingStoreAllocation, WlBuffer>,

    views: Arc<Mutex<HashMap<ViewId, FlutterWaylandSurface>>>,
}

impl NellyCompositor {
    /// Embedder callbacks are invoked on the Flutter Engine's own threads.
    /// As such, they don't run in the event loop, and any requests they make
    /// on Wayland objects may not be flushed immediately. This is because those
    /// requests are queued up, and the event loop flushes them at the end of the
    /// event loop iteration, and not after each event.
    ///
    /// This means that if the event queue is just waiting, it will block indefinitely
    /// until an event occurs. And if no events occur, it will think that nothing is happening.
    /// And if nothing is happening, it will not flush the queue.
    ///
    /// To mitigate this, we send a ping message to the event loop to wake it up.
    /// This is a no-op, but it will cause the event loop to complete an iteration and flush the queue.
    fn ping_queue(&self) {
        self.msg.send(EmbedderMessage::Ping).unwrap();
    }
}

pub enum FlutterWaylandSurface {
    XdgToplevel(Window),
    XdgPopup(Popup),
    // SessionLock(SessionLockSurface),
    // Layer(LayerSurface),
}

impl WaylandSurface for FlutterWaylandSurface {
    fn surface(&self) -> &Surface {
        match self {
            FlutterWaylandSurface::XdgToplevel(toplevel) => toplevel.surface(),
            FlutterWaylandSurface::XdgPopup(popup) => popup.surface(),
        }
    }
}

impl From<Window> for FlutterWaylandSurface {
    fn from(window: Window) -> Self {
        FlutterWaylandSurface::XdgToplevel(window)
    }
}

impl From<Popup> for FlutterWaylandSurface {
    fn from(popup: Popup) -> Self {
        FlutterWaylandSurface::XdgPopup(popup)
    }
}

// impl From<SessionLockSurface> for FlutterWaylandSurface {
//     fn from(lock: SessionLockSurface) -> Self {
//         FlutterWaylandSurface::SessionLock(lock)
//     }
// }

// impl From<LayerSurface> for FlutterWaylandSurface {
//     fn from(layer: LayerSurface) -> Self {
//         FlutterWaylandSurface::Layer(layer)
//     }
// }

impl fluster::CompositorHandler for NellyCompositor {
    fn create_backing_store(&mut self, config: BackingStoreConfig) -> Option<BackingStore> {
        if config.size.width.fract() != 0.0 || config.size.height.fract() != 0.0 {
            error!(
                "backing store size is not pixel aligned: {:?}",
                config.size.height
            );
            return None;
        }

        if config.size.width.is_sign_negative() || config.size.height.is_sign_negative() {
            error!("backing store size is negative: {:?}", config.size.height);
            return None;
        }

        #[expect(clippy::cast_possible_truncation, reason = "checked")]
        let width = config.size.width as i32;
        #[expect(clippy::cast_possible_truncation, reason = "checked")]
        let height = config.size.height as i32;

        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_possible_wrap,
            reason = "Wayland requires i32. can't do anything about it. also, it's checked"
        )]
        let stride = width * PixelFormat.bytes() as i32;

        let pool = SinglePool::new(
            width,
            height,
            stride,
            PixelFormat.into(),
            &self.qh,
            &self.wl_shm,
        )
        .inspect_err(|e| {
            error!("failed to create a pool: {:?}", e);
        })
        .ok()?;

        let allocation = pool.mmap().as_mut_ptr();

        self.buffers
            .insert(BackingStoreAllocation(allocation), pool.buffer().clone());

        self.ping_queue();

        #[allow(clippy::cast_sign_loss, reason = "checked")]
        Some(BackingStore::Software(SoftwareBackingStore {
            allocation,
            row_bytes: stride as usize,
            height: height as usize,
            pixel_format: PixelFormat.into(),
        }))
    }

    fn collect_backing_store(&mut self, backing_store: BackingStore) -> bool {
        #[expect(
            clippy::single_match_else,
            reason = "will add more backings stores later"
        )]
        match backing_store {
            BackingStore::Software(SoftwareBackingStore { allocation, .. }) => {
                // drop glue is in an Arc that the WlBuffer still holds a strong reference to
                self.buffers.remove(&BackingStoreAllocation(allocation));
                self.ping_queue();
                true
            }
            _ => {
                error!("collect backing store but not software");
                false
            }
        }
    }

    fn present_view(&mut self, view_id: ViewId, layers: &[fluster::Layer]) -> bool {
        let views = self.views.lock().unwrap();
        let Some(view) = views.get(&view_id) else {
            error!("flutter gave me a view id i don't know about");
            return false;
        };

        view.request_throttled_frame_callback(&self.qh);

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

        let &BackingStore::Software(SoftwareBackingStore { allocation, .. }) = backing_store else {
            error!("flutter gave me a backing store i can't handle (i only ever submitted software backing stores)");
            return false;
        };

        let Some(buffer) = self.buffers.get(&BackingStoreAllocation(allocation)) else {
            error!("flutter gave me a software backing store i didn't submit");
            return false;
        };

        view.attach(Some(buffer), 0, 0);

        for rect in &present_info.paint_region.regions {
            if rect.top != 0.0 || rect.left != 0.0 {
                error!("paint region {rect:?} is not at 0,0"); // TODO: is `right` and `bottom` meant to be `x+width` and `y+height`?
            }
            let (x, y) = (rect.left, rect.top);
            let (width, height) = (rect.right, rect.bottom);

            if x.fract() != 0.0 || y.fract() != 0.0 || width.fract() != 0.0 || height.fract() != 0.0
            {
                error!("paint region {rect:?} is not pixel aligned");
                return false;
            }

            #[expect(
                clippy::cast_possible_truncation,
                reason = "Wayland requires i32. can't do anything about it."
            )]
            view.damage_buffer(x as i32, y as i32, width as i32, height as i32);
        }

        view.viewport()
            .set_source(0.0, 0.0, layer.size.width, layer.size.height);

        #[expect(clippy::cast_possible_truncation)] // TODO: is this correct?
        view.viewport().set_destination(
            (layer.size.width / view.scale_factor()).round() as i32,
            (layer.size.height / view.scale_factor()).round() as i32,
        );

        view.commit();

        self.ping_queue();

        true
    }
}

pub fn init(
    assets_path: &Path,
    app_library: Option<&Path>,
    config: &Arc<Mutex<Config>>,
    loop_handle: &LoopHandle<'static, Nelly>,
    shm: &Shm,
    qh: &QueueHandle<Nelly>,
    views: Arc<Mutex<HashMap<ViewId, FlutterWaylandSurface>>>,
) -> anyhow::Result<Engine> {
    let aot_data = app_library
        .map(Path::to_path_buf)
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
        .insert_source(chan, |msg, (), nelly| {
            match msg {
                calloop::channel::Event::Msg(msg) => {
                    match msg {
                        EmbedderMessage::Ping => {}
                        EmbedderMessage::Vsync(vsync_baton) => {
                            // debug!("vsync: {:?}", vsync_baton);
                            nelly
                                .engine()
                                .on_vsync(
                                    vsync_baton,
                                    Engine::get_current_time(),
                                    Engine::get_current_time(),
                                )
                                .unwrap();
                        }
                        EmbedderMessage::PlatformMessage(msg, response) => {
                            match msg.run(nelly) {
                                Ok(response_data) => {
                                    response.send(&response_data).unwrap();
                                }
                                Err(e) => {
                                    error!("failed to run platform message: {e:?}");
                                    response.send(&[]).unwrap(); // send empty response, avoids memory leak
                                }
                            }
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
            assets_path,
            aot_data,
            icu_data_path: Path::new(crate::engine_meta::ICUDTL_DAT),

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
                    qh: qh.clone(),
                    wl_shm: shm.wl_shm().clone(),
                    buffers: HashMap::new(),
                    views,
                }),
            }),
            dart_entrypoint_argv: &[],
            log_tag: c"flutter".into(),
            dart_old_gen_heap_size: 0,
            handler: Box::new(Handler {
                config: config.clone(),
                msg: send.clone(),
            }),
            compute_platform_resolved_locale: None,
        },
    )?;

    debug!("engine initialized");

    Ok(engine)
}
