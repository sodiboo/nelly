//! A raw shared memory pool handler.
//!
//! This is intended as a safe building block for higher level shared memory pool abstractions and is not
//! encouraged for most library users.

use rustix::{
    io::Errno,
    shm::{Mode, ShmOFlags},
};
use std::{
    fs::File,
    io,
    os::unix::prelude::{AsFd, OwnedFd},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::debug;

use memmap2::MmapRaw;
use smithay_client_toolkit::reexports::client::{
    delegate_noop,
    protocol::{
        wl_buffer::{self, WlBuffer},
        wl_shm, wl_shm_pool,
    },
    Connection, Dispatch, Proxy, QueueHandle,
};

use crate::nelly::Nelly;

#[derive(Debug)]
pub struct SinglePool {
    pool: wl_shm_pool::WlShmPool,
    buffer: WlBuffer,
    backing: Arc<BufferBacking>,
}

#[derive(Debug)]
pub struct BufferBacking {
    pub mem_file: File,
    pub mmap: MmapRaw,
}

impl Drop for BufferBacking {
    fn drop(&mut self) {
        debug!("Dropping buffer backing");
    }
}

delegate_noop!(Nelly: wl_shm_pool::WlShmPool); // no events

impl Dispatch<WlBuffer, Arc<BufferBacking>> for Nelly {
    fn event(
        _: &mut Self,
        proxy: &WlBuffer,
        event: <WlBuffer as Proxy>::Event,
        _: &Arc<BufferBacking>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_buffer::Event::Release => {
                proxy.destroy();
            }
            _ => unreachable!(),
        }
    }
}

impl SinglePool {
    pub fn new<D>(
        width: i32,
        height: i32,
        stride: i32,
        format: wl_shm::Format,
        qh: &QueueHandle<D>,
        shm: &wl_shm::WlShm,
    ) -> io::Result<SinglePool>
    where
        D: Dispatch<WlBuffer, Arc<BufferBacking>> + 'static,
        D: Dispatch<wl_shm_pool::WlShmPool, ()> + 'static,
    {
        let size = stride * height;
        let shm_fd = SinglePool::create_shm_fd()?;
        let mem_file = File::from(shm_fd);
        mem_file.set_len(size as u64)?;

        let pool = shm.create_pool(mem_file.as_fd(), size, qh, ());
        let mmap = MmapRaw::map_raw(&mem_file)?;

        let backing = Arc::new(BufferBacking { mem_file, mmap });

        let buffer = pool.create_buffer(0, width, height, stride, format, qh, backing.clone());

        Ok(SinglePool {
            pool,
            buffer,
            backing,
        })
    }

    /// Returns a reference to the underlying shared memory file using the memmap2 crate.
    pub fn mmap(&self) -> &MmapRaw {
        &self.backing.mmap
    }

    pub fn buffer(&self) -> &WlBuffer {
        &self.buffer
    }
}

impl SinglePool {
    fn create_shm_fd() -> io::Result<OwnedFd> {
        #[cfg(target_os = "linux")]
        {
            match SinglePool::create_memfd() {
                Ok(fd) => return Ok(fd),

                // Not supported, use fallback.
                Err(Errno::NOSYS) => (),

                Err(err) => return Err(Into::<io::Error>::into(err)),
            };
        }

        let time = SystemTime::now();
        let mut mem_file_handle = format!(
            "/nelly-{}",
            time.duration_since(UNIX_EPOCH).unwrap().subsec_nanos()
        );

        loop {
            let flags = ShmOFlags::CREATE | ShmOFlags::EXCL | ShmOFlags::RDWR;

            let mode = Mode::RUSR | Mode::WUSR;

            match rustix::shm::shm_open(mem_file_handle.as_str(), flags, mode) {
                Ok(fd) => match rustix::shm::shm_unlink(mem_file_handle.as_str()) {
                    Ok(_) => return Ok(fd),

                    Err(errno) => {
                        return Err(errno.into());
                    }
                },

                Err(Errno::EXIST) => {
                    // Change the handle if we happen to be duplicate.
                    let time = SystemTime::now();

                    mem_file_handle = format!(
                        "/nelly-{}",
                        time.duration_since(UNIX_EPOCH).unwrap().subsec_nanos()
                    );

                    continue;
                }

                Err(Errno::INTR) => continue,

                Err(err) => return Err(err.into()),
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn create_memfd() -> rustix::io::Result<OwnedFd> {
        use rustix::fs::{MemfdFlags, SealFlags};

        loop {
            let flags = MemfdFlags::ALLOW_SEALING | MemfdFlags::CLOEXEC;

            match rustix::fs::memfd_create(c"nelly", flags) {
                Ok(fd) => {
                    // We only need to seal for the purposes of optimization, ignore the errors.
                    let _ = rustix::fs::fcntl_add_seals(&fd, SealFlags::SHRINK | SealFlags::SEAL);
                    return Ok(fd);
                }

                Err(Errno::INTR) => continue,

                Err(err) => return Err(err),
            }
        }
    }
}

impl Drop for SinglePool {
    fn drop(&mut self) {
        self.pool.destroy();
    }
}
