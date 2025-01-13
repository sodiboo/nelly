use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
    thread::ThreadId,
    time::{Duration, Instant},
};

use volito::{
    AOTData, AOTDataSource, BackingStore, BackingStoreConfig, CustomTaskRunners, Engine,
    LayerContent, ProjectArgs, SoftwareBackingStore, SoftwareRendererConfig, TaskRunnerDescription,
    ViewId,
};
use smithay_client_toolkit::{
    reexports::{
        calloop::{
            self,
            channel::{channel, Sender},
            timer::{TimeoutAction, Timer},
        },
        client::{
            protocol::{wl_buffer::WlBuffer, wl_shm},
            QueueHandle,
        },
    },
    shm::Shm,
};
use tracing::{debug, error, info, trace};

use crate::{
    config::Config,
    nelly::Nelly,
    platform_message::AnyPlatformRequest,
    pool::SinglePool,
    shell::{
        compositor::Surface,
        layer::WlrLayerSurface,
        xdg::{popup::XdgPopupSurface, window::XdgToplevelSurface},
        WaylandSurface,
    },
};

enum EmbedderMessage {
    Vsync(volito::VsyncBaton),
    PlatformMessage(AnyPlatformRequest, volito::PlatformMessageResponse),
    Task(Instant, volito::Task),
}

pub struct Handler {
    config: Arc<Mutex<Config>>,
    msg: Sender<EmbedderMessage>,
    signal: calloop::LoopSignal,
}

struct TaskRunner {
    config: Arc<Mutex<Config>>,
    msg: Sender<EmbedderMessage>,
    thread: ThreadId,
}

impl volito::TaskRunnerHandler for TaskRunner {
    fn runs_task_on_current_thread(&self) -> bool {
        self.thread == std::thread::current().id()
    }

    fn post_task(&self, target_time: Duration, task: volito::Task) {
        let now = volito::Engine::get_current_time();
        let deadline = if target_time > now {
            let delta = target_time - now;
            Instant::now() + delta
        } else {
            Instant::now()
        };
        self.msg
            .send(EmbedderMessage::Task(deadline, task))
            .unwrap();
    }
}

impl volito::SoftwareRendererHandler for Handler {
    fn surface_present(&mut self, _: *const u8, _: usize, _: usize) -> bool {
        error!("surface present; should never be called because we use a FlutterCompositor?");
        false
    }
}

impl volito::EngineHandler for Handler {
    fn platform_message(
        &mut self,
        channel: &std::ffi::CStr,
        message: &[u8],
        response: volito::PlatformMessageResponse,
    ) {
        match AnyPlatformRequest::decode(channel, message) {
            Ok(message) => {
                info!("decoded platform message: {message:?}");
                self.msg
                    .send(EmbedderMessage::PlatformMessage(message, response))
                    .unwrap()
            }
            Err(e) => {
                // flutter has a bunch of default channels that we don't care about or handle
                if !channel.to_bytes().starts_with(b"flutter/") {
                    error!("{e:?}");
                }
                response.send(&[]).unwrap(); // send empty response, avoids memory leak
            }
        }

        self.signal.wakeup();
    }

    fn vsync(&mut self, baton: volito::VsyncBaton) {
        self.msg.send(EmbedderMessage::Vsync(baton)).unwrap();
    }

    fn update_semantics(&mut self, _update: volito::SemanticsUpdate) {
        debug!("update semantics");
    }

    fn log_message(&mut self, tag: &std::ffi::CStr, message: &std::ffi::CStr) {
        let tag = tag.to_string_lossy();
        let message = message.to_string_lossy();
        ::dart_tracing::log_info_with_tag(&tag, &message);
    }

    fn on_pre_engine_restart(&mut self) {
        debug!("pre engine restart");
    }

    fn channel_update(&mut self, channel: &std::ffi::CStr, listening: bool) {
        trace!("channel update: {channel:?}, {listening}");
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

impl From<PixelFormat> for volito::SoftwarePixelFormat {
    fn from(PixelFormat: PixelFormat) -> Self {
        volito::SoftwarePixelFormat::BGRA8888 // [u8; 4]
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
    signal: calloop::LoopSignal,

    qh: QueueHandle<Nelly>,
    wl_shm: wl_shm::WlShm,

    buffers: HashMap<BackingStoreAllocation, WlBuffer>,

    views: Arc<Mutex<HashMap<ViewId, FlutterWaylandSurface>>>,
}

pub enum FlutterWaylandSurface {
    WlrLayer(WlrLayerSurface),
    XdgToplevel(XdgToplevelSurface),
    XdgPopup(XdgPopupSurface),
    // SessionLock(SessionLockSurface),
    // Layer(LayerSurface),
}

impl WaylandSurface for FlutterWaylandSurface {
    fn surface(&self) -> &Surface {
        match self {
            FlutterWaylandSurface::WlrLayer(layer) => layer.surface(),
            FlutterWaylandSurface::XdgToplevel(toplevel) => toplevel.surface(),
            FlutterWaylandSurface::XdgPopup(popup) => popup.surface(),
        }
    }
}

impl From<WlrLayerSurface> for FlutterWaylandSurface {
    fn from(layer: WlrLayerSurface) -> Self {
        FlutterWaylandSurface::WlrLayer(layer)
    }
}

impl From<XdgToplevelSurface> for FlutterWaylandSurface {
    fn from(window: XdgToplevelSurface) -> Self {
        FlutterWaylandSurface::XdgToplevel(window)
    }
}

impl From<XdgPopupSurface> for FlutterWaylandSurface {
    fn from(popup: XdgPopupSurface) -> Self {
        FlutterWaylandSurface::XdgPopup(popup)
    }
}

// impl From<SessionLockSurface> for FlutterWaylandSurface {
//     fn from(lock: SessionLockSurface) -> Self {
//         FlutterWaylandSurface::SessionLock(lock)
//     }
// }

impl volito::CompositorHandler for NellyCompositor {
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

        self.signal.wakeup();

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
                self.signal.wakeup();
                true
            }
            _ => {
                error!("collect backing store but not software");
                false
            }
        }
    }

    fn present_view(&mut self, view_id: ViewId, layers: &[volito::Layer]) -> bool {
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

        self.signal.wakeup();

        true
    }
}

pub fn init(
    assets_path: &Path,
    app_library: Option<&Path>,
    config: &Arc<Mutex<Config>>,
    event_loop: &calloop::EventLoop<'static, Nelly>,
    shm: &Shm,
    qh: &QueueHandle<Nelly>,
    views: Arc<Mutex<HashMap<ViewId, FlutterWaylandSurface>>>,
) -> anyhow::Result<Engine> {
    let platform_thread = std::thread::current().id();

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

    event_loop
        .handle()
        .insert_source(chan, move |msg, (), nelly| {
            match msg {
                calloop::channel::Event::Msg(msg) => {
                    match msg {
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
                        EmbedderMessage::Task(deadline, task) => {
                            let mut task = Some(task);
                            nelly
                                .loop_handle
                                .insert_source(
                                    Timer::from_deadline(deadline),
                                    move |_, (), nelly| {
                                        assert!(std::thread::current().id() == platform_thread);
                                        nelly.engine().run_task(task.take().unwrap()).unwrap();
                                        TimeoutAction::Drop
                                    },
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
                signal: event_loop.get_signal(),
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
            custom_task_runners: Some(CustomTaskRunners {
                platform_task_runner: Some(TaskRunnerDescription {
                    identifier: 1,
                    handler: Box::new(TaskRunner {
                        config: config.clone(),
                        msg: send.clone(),
                        thread: platform_thread,
                    }),
                }),
                render_task_runner: None,
                set_thread_priority: None,
            }),
            shutdown_dart_vm_when_done: true,
            compositor: Some(volito::Compositor {
                avoid_backing_store_cache: true,
                handler: Box::new(NellyCompositor {
                    config: config.clone(),
                    msg: send.clone(),
                    signal: event_loop.get_signal(),
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
                signal: event_loop.get_signal(),
            }),
            compute_platform_resolved_locale: None,
        },
    )?;

    debug!("engine initialized");

    Ok(engine)
}
