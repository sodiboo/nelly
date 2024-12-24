use std::{
    fmt,
    sync::atomic::{AtomicU64, Ordering},
};

pub struct AtomicF64 {
    storage: AtomicU64,
}
impl AtomicF64 {
    pub fn new(value: f64) -> Self {
        let as_u64 = value.to_bits();
        Self {
            storage: AtomicU64::new(as_u64),
        }
    }
    pub fn store(&self, value: f64) {
        let as_u64 = value.to_bits();
        self.storage.store(as_u64, Ordering::SeqCst)
    }
    pub fn load(&self) -> f64 {
        let as_u64 = self.storage.load(Ordering::SeqCst);
        f64::from_bits(as_u64)
    }
}

impl fmt::Display for AtomicF64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.load(), f)
    }
}

impl fmt::Debug for AtomicF64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AtomicF64").field(&self.load()).finish()
    }
}
