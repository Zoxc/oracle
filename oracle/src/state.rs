use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json;
use std::fs;
use std::net::Ipv4Addr;
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Configuration {
    pub web_port: u16,
    pub ping_interval: u32,
}

impl Configuration {
    pub fn load() -> Configuration {
        serde_json::from_str(&fs::read_to_string("data/config.json").unwrap()).unwrap()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Device {
    pub id: u32,
    pub name: Option<String>,
    pub ipv4: Option<Ipv4Addr>,
}

pub struct StateFields {
    pub config: Configuration,
    pub devices: Vec<Device>,
}

impl StateFields {
    pub fn new_device_id(&self) -> u32 {
        for i in 0..=(u32::MAX) {
            if self.devices.iter().find(|d| d.id == i).is_none() {
                return i;
            }
        }
        panic!()
    }

    pub fn save_config(&self) {
        fs::write(
            "data/config.json",
            serde_json::to_string_pretty(&self.config).unwrap(),
        )
        .unwrap()
    }

    pub fn save_devices(&self) {
        fs::write(
            "data/devices.json",
            serde_json::to_string_pretty(&self.devices).unwrap(),
        )
        .unwrap()
    }
}

pub type State = Arc<Mutex<StateFields>>;

pub fn load() -> State {
    let devices = serde_json::from_str(&fs::read_to_string("data/devices.json").unwrap()).unwrap();
    let config = Configuration::load();
    Arc::new(Mutex::new(StateFields { config, devices }))
}
