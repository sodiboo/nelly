use std::sync::{Arc, Mutex};

pub struct Config {}

impl Config {
    pub fn load() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {}))
    }
}
