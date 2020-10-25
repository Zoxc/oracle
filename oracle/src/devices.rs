use crate::monitor::{self, CancelToken, DeviceStatus};
use crate::ping::Ping;
use crate::state::Conf;
use futures::{SinkExt, StreamExt};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{self, json};
use std::fs;
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::{broadcast, mpsc, oneshot};
use warp::ws;
use warp::{filters::BoxedFilter, Filter, Reply};

pub type DeviceId = u32;

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct DeviceConf {
    pub id: DeviceId,
    pub name: Option<String>,
    pub ipv4: Option<Ipv4Addr>,
    #[serde(default)]
    pub snmp: bool,
    #[serde(default)]
    pub snmp_community: Option<String>,
}

impl DeviceConf {
    pub fn desc(&self) -> String {
        if let Some(name) = &self.name {
            name.clone()
        } else if let Some(ipv4) = self.ipv4 {
            ipv4.to_string()
        } else {
            format!("<device #{}>", self.id)
        }
    }
}

#[derive(Debug)]
pub struct Device {
    pub conf: DeviceConf,
    pub ipv4_status: DeviceStatus,
    pub ipv4_status_since: Option<SystemTime>,
    pub ipv4_monitor: Option<CancelToken>,
}

impl Device {
    pub fn new(id: DeviceId) -> Self {
        let mut conf: DeviceConf = Default::default();
        conf.id = id;
        Self {
            conf,
            ipv4_status: DeviceStatus::Unknown,
            ipv4_status_since: None,
            ipv4_monitor: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum DeviceChange {
    Added(DeviceId),
    Removed(DeviceId),
    IPv4Status {
        device: DeviceId,
        old: DeviceStatus,
        new: DeviceStatus,
        since: Option<SystemTime>,
    },
}

pub struct Devices {
    pub list: Vec<Device>,
    pub changes: broadcast::Sender<DeviceChange>,
    pub conf: Conf,
    pub ping: Ping,
}

impl Devices {
    pub fn add(&mut self, conf: DeviceConf, devices: Arc<Mutex<Devices>>) {
        let id = conf.id;
        self.list.push(Device::new(id));
        self.change(id, conf, devices);
        //devices.changes.send(DeviceChange::Added(device.id)).ok();
    }

    pub fn remove(&mut self, id: DeviceId, devices: Arc<Mutex<Devices>>) {
        self.change(id, Default::default(), devices);
        let index = self.device_index(id);
        index.map(|index| self.list.remove(index));
        //self.changes.send(DeviceChange::Removed(id)).ok();
    }

    pub fn change(&mut self, id: DeviceId, conf: DeviceConf, devices: Arc<Mutex<Devices>>) {
        let index = if let Some(index) = self.device_index(id) {
            index
        } else {
            return;
        };
        let old_conf = self.list[index].conf.clone();
        let device = &mut self.list[index];

        if old_conf.ipv4 != conf.ipv4 {
            device.ipv4_monitor.as_ref().map(|token| token.cancel());
            device.ipv4_monitor = None;

            if let Some(ipv4) = conf.ipv4 {
                let token = CancelToken::new();
                device.ipv4_monitor = Some(token.clone());
                tokio::spawn(monitor::device_monitor(
                    devices,
                    ipv4,
                    self.ping.clone(),
                    device.ipv4_status,
                    id,
                    token,
                ));
            }
        }

        device.conf = conf;
    }

    pub fn new_device_id(&self) -> DeviceId {
        // TODO: Race condition. Old Ids may still be referenced by tasks (and browsers)
        for i in 0..=(u32::MAX) {
            if self.list.iter().find(|d| d.conf.id == i).is_none() {
                return i;
            }
        }
        panic!()
    }

    pub fn save(&self) {
        let confs: Vec<_> = self.list.iter().map(|device| &device.conf).collect();
        fs::write(
            "data/devices.json",
            serde_json::to_string_pretty(&confs).unwrap(),
        )
        .unwrap()
    }

    pub fn device_index(&self, id: DeviceId) -> Option<usize> {
        self.list
            .iter()
            .enumerate()
            .find(|d| d.1.conf.id == id)
            .map(|d| d.0)
    }

    pub fn device(&self, id: DeviceId) -> &Device {
        &self.list[self.device_index(id).unwrap()]
    }

    pub fn device_mut(&mut self, id: DeviceId) -> &mut Device {
        let index = self.device_index(id).unwrap();
        &mut self.list[index]
    }
}

pub fn webserver(devices: Arc<Mutex<Devices>>) -> BoxedFilter<(impl Reply,)> {
    let devices_ = devices.clone();
    let list_devices = warp::path("devices")
        .and(warp::get())
        .and(warp::path::end())
        .map(move || {
            let devices = devices_.lock();
            let confs: Vec<_> = devices.list.iter().map(|device| &device.conf).collect();
            serde_json::to_string(&confs).unwrap()
        });

    let devices_ = devices.clone();
    let add = warp::path("device")
        .and(warp::post())
        .and(warp::path::end())
        .and(warp::body::json())
        .map(move |device: DeviceConf| {
            //let ipv4 = from_map(&config, "ipv4");

            println!("device: {:?}", device);

            let mut devices = devices_.lock();
            devices.add(device, devices_.clone());
            devices.save();

            ""
        });

    let devices_ = devices.clone();
    let remove = warp::path!("device" / u32)
        .and(warp::delete())
        .map(move |id| {
            let mut devices = devices_.lock();
            devices.remove(id, devices_.clone());
            devices.save();

            ""
        });

    let devices_ = devices.clone();
    let status = warp::path!("devices" / "status")
        .and(warp::ws())
        .map(move |ws: ws::Ws| {
            let devices_ = devices_.clone();
            ws.on_upgrade(|websocket| async move {
                let (mut tx, mut rx) = websocket.split();

                let (initial, mut changes) = {
                    let devices = devices_.lock();
                    let initial: Vec<_> = devices
                        .list
                        .iter()
                        .map(|device| {
                            json!({"id": device.conf.id, "status": device.ipv4_status, "since": device.ipv4_status_since})
                        })
                        .collect();
                    let changes = devices.changes.subscribe();
                    (initial, changes)
                };

                tx.send(ws::Message::text(serde_json::to_string(&initial).unwrap()))
                    .await
                    .ok(); // May fail due to the websocket closing

                loop {
                    tokio::select! {
                        Some(Ok(msg)) = rx.next() => {
                            if msg.is_close() {
                                break
                            }
                        },
                        Ok(change) = changes.recv() => {
                            let (device, status, since) = match change {
                                DeviceChange::IPv4Status { device, old, new, since } => {
                                    (device, new, since)
                                }
                                _ => continue,
                            };

                            let val = json!([{"id": device, "status": status, "since": since}]);

                            tx.send(ws::Message::text(
                                serde_json::to_string(&val).unwrap(),
                            )).await.ok(); // May fail due to the websocket closing
                        },
                        else => {
                            break
                        }
                    };
                }
            })
        });

    list_devices.or(add).or(remove).or(status).boxed()
}

pub fn load(conf: Conf) -> Arc<Mutex<Devices>> {
    let ping = Ping::new();

    let (changes, _) = broadcast::channel(1000);
    let devices = Arc::new(Mutex::new(Devices {
        list: Vec::new(),
        changes,
        conf,
        ping,
    }));

    let list: Vec<DeviceConf> =
        serde_json::from_str(&fs::read_to_string("data/devices.json").unwrap()).unwrap();

    {
        let mut lock = devices.lock();

        for device in list {
            lock.add(device, devices.clone());
        }
    }

    devices
}
