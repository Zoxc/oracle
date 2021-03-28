use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json;
use std::fs;
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Smtp {
    pub server: String,
    pub from: String,
    pub user: String,
    pub password: String,
    pub recievers: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub web_port: u16,
    pub ping_interval: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub name: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Configuration {
    #[serde(flatten)]
    pub config: Config,
    pub smtp: Option<Smtp>,
    pub users: Vec<User>,
}

impl Configuration {
    pub fn load() -> Configuration {
        serde_json::from_str(&fs::read_to_string("data/config.json").unwrap()).unwrap()
    }

    pub fn save(&self) {
        fs::write(
            "data/config.json",
            serde_json::to_string_pretty(&self).unwrap(),
        )
        .unwrap()
    }
}

pub type State = Arc<Mutex<Configuration>>;
pub type Conf = Arc<Mutex<Configuration>>;

pub fn load() -> State {
    Arc::new(Mutex::new(Configuration::load()))
}
