use crate::state::Conf;
use crate::{log::Kind, log::Log, ping::Ping};
use crate::{
    monitor::{self, CancelToken},
    notifier,
};
use futures::{SinkExt, StreamExt};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{self, json};
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::SystemTime;
use std::{fs, time::Instant};
use tokio::{
    spawn,
    sync::{broadcast, mpsc},
};
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

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Eq, PartialEq)]
pub enum ServiceStatus {
    Up,
    Down,
}

#[derive(Debug, Default)]
pub struct Service {
    pub status: Option<(ServiceStatus, SystemTime)>,
    pub monitor: Option<CancelToken>,
}

#[derive(Debug)]
pub struct Device {
    pub conf: Mutex<DeviceConf>,

    // `conf` lock taken before `icmpv4`
    pub icmpv4: Mutex<Service>,
}

impl Device {
    pub fn new(id: DeviceId) -> Self {
        let mut conf: DeviceConf = Default::default();
        conf.id = id;
        Self {
            conf: Mutex::new(conf),
            icmpv4: Default::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum DeviceChange {
    Added(DeviceId),
    Removed(DeviceId),
    IPv4Status {
        device: DeviceId,
        old: Option<(ServiceStatus, SystemTime)>,
        new: Option<(ServiceStatus, SystemTime)>,
    },
}

pub struct Devices {
    pub list: Mutex<Vec<Arc<Device>>>,
    pub changes: broadcast::Sender<DeviceChange>,
    pub last_email: Mutex<Option<Instant>>,
    pub notifiers: Mutex<Vec<mpsc::Sender<DeviceChange>>>,
    pub conf: Conf,
    pub ping: Ping,
    pub log: Arc<Log>,
}

impl Devices {
    pub async fn notify(self: &Arc<Self>, change: DeviceChange) {
        let (device, status) = match change.clone() {
            DeviceChange::IPv4Status {
                device,
                old: Some(_),
                new: Some(new),
            } => (device, new),
            _ => return,
        };

        let desc = self.device(device).conf.lock().desc();

        match status.0 {
            ServiceStatus::Up => self.log.log(Kind::Note, &format!("Device {} is up", desc)),
            ServiceStatus::Down => self
                .log
                .log(Kind::Error, &format!("Device {} is down", desc)),
        }

        let notifiers = self.notifiers.lock().clone();

        for mut notifier in notifiers {
            notifier.send(change.clone()).await.unwrap();
        }
    }

    pub fn add(self: &Arc<Self>, conf: DeviceConf) {
        let id = conf.id;
        self.list.lock().push(Arc::new(Device::new(id)));
        self.change(id, conf);
        self.changes.send(DeviceChange::Added(id)).ok();
    }

    pub fn remove(self: &Arc<Self>, id: DeviceId) {
        let index = self.device_index(id);
        self.change(id, Default::default());
        index.map(|index| self.list.lock().remove(index));
        self.changes.send(DeviceChange::Removed(id)).ok();
    }

    pub fn change(self: &Arc<Self>, id: DeviceId, conf: DeviceConf) {
        let device = self.device(id);
        let mut device_conf = device.conf.lock();
        let old_conf = device_conf.clone();

        if old_conf.ipv4 != conf.ipv4 {
            let mut icmpv4 = device.icmpv4.lock();
            icmpv4.monitor.as_ref().map(|token| token.cancel());
            icmpv4.monitor = None;

            if let Some(ipv4) = conf.ipv4 {
                let token = CancelToken::new();
                icmpv4.monitor = Some(token.clone());
                tokio::spawn(monitor::device_monitor(
                    self.clone(),
                    device.clone(),
                    ipv4,
                    token,
                ));
            } else {
                icmpv4.status = None;
            }
        }

        *device_conf = conf;
    }

    pub fn new_device_id(&self) -> DeviceId {
        // TODO: Race condition. Old Ids may still be referenced by tasks (and browsers)
        for i in 0..=(u32::MAX) {
            if self
                .list
                .lock()
                .iter()
                .find(|d| d.conf.lock().id == i)
                .is_none()
            {
                return i;
            }
        }
        panic!()
    }

    pub fn save(&self) {
        let confs: Vec<_> = self
            .list
            .lock()
            .iter()
            .map(|device| device.conf.lock().clone())
            .collect();
        fs::write(
            "data/devices.json",
            serde_json::to_string_pretty(&confs).unwrap(),
        )
        .unwrap()
    }

    pub fn device_index(&self, id: DeviceId) -> Option<usize> {
        self.list
            .lock()
            .iter()
            .enumerate()
            .find(|d| d.1.conf.lock().id == id)
            .map(|d| d.0)
    }

    pub fn device(&self, id: DeviceId) -> Arc<Device> {
        let index = self.device_index(id).unwrap();
        self.list.lock()[index].clone()
    }
}

pub fn webserver(devices: Arc<Devices>) -> BoxedFilter<(impl Reply,)> {
    let devices_ = devices.clone();
    let list_devices = warp::path("devices")
        .and(warp::get())
        .and(warp::path::end())
        .map(move || {
            let confs: Vec<_> = devices_
                .list
                .lock()
                .iter()
                .map(|device| device.conf.lock().clone())
                .collect();
            serde_json::to_string(&confs).unwrap()
        });

    let devices_ = devices.clone();
    let add = warp::path("device")
        .and(warp::post())
        .and(warp::path::end())
        .and(warp::body::json())
        .map(move |mut device: DeviceConf| {
            device.id = devices_.new_device_id();
            devices_.add(device);
            devices_.save();

            ""
        });

    let devices_ = devices.clone();
    let remove = warp::path!("device" / u32)
        .and(warp::delete())
        .map(move |id| {
            devices_.remove(id);
            devices_.save();

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
                    let initial: Vec<_> = devices_
                        .list
                        .lock()
                        .iter()
                        .map(|device| {
                            let id = device.conf.lock().id;
                            let icmpv4 = device.icmpv4.lock();
                            json!({"id": id, "status": icmpv4.status})
                        })
                        .collect();
                    let changes = devices_.changes.subscribe();
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
                            let (device, status) = match change {
                                DeviceChange::IPv4Status { device, old: _, new } => {
                                    (device, new)
                                }
                                _ => continue,
                            };

                            let val = json!([{"id": device, "status": status}]);

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

pub fn load(conf: Conf, log: Arc<Log>) -> Arc<Devices> {
    let ping = Ping::new();

    let (changes, _) = broadcast::channel(1000);

    let receivers = conf
        .lock()
        .smtp
        .as_ref()
        .map(|smtp| smtp.recievers.clone())
        .unwrap_or_default();

    let devices = Arc::new(Devices {
        list: Mutex::new(Vec::new()),
        changes,
        conf: conf.clone(),
        ping,
        log: log.clone(),
        last_email: Mutex::new(Some(Instant::now())),
        notifiers: Mutex::new(Vec::new()),
    });

    for receiver in receivers {
        let (tx, rx) = mpsc::channel(1000);

        spawn(notifier::notifier(
            conf.clone(),
            devices.clone(),
            log.clone(),
            receiver.clone(),
            rx,
        ));

        devices.notifiers.lock().push(tx);
    }

    let list: Vec<DeviceConf> =
        serde_json::from_str(&fs::read_to_string("data/devices.json").unwrap()).unwrap();

    for device in list {
        devices.add(device);
    }

    devices
}
