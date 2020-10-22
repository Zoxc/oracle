use parking_lot::Mutex;
use ron;
use serde::{Deserialize, Serialize};
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
        ron::de::from_str(&fs::read_to_string("data/config.ron").unwrap()).unwrap()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Device {
    pub id: u32,
    pub ipv4: Ipv4Addr,
}

pub struct StateFields {
    pub config: Configuration,
    pub devices: Vec<Device>,
}

impl StateFields {
    pub fn save_config(&self) {
        fs::write(
            "data/config.ron",
            ron::ser::to_string_pretty(&self.config, ron::ser::PrettyConfig::new()).unwrap(),
        )
        .unwrap()
    }

    pub fn save_devices(&self) {
        fs::write(
            "data/devices.ron",
            ron::ser::to_string_pretty(&self.devices, ron::ser::PrettyConfig::new()).unwrap(),
        )
        .unwrap()
    }
}

pub type State = Arc<Mutex<StateFields>>;

pub fn load() -> State {
    let devices = ron::de::from_str(&fs::read_to_string("data/devices.ron").unwrap()).unwrap();
    let config = Configuration::load();
    Arc::new(Mutex::new(StateFields { config, devices }))
}
