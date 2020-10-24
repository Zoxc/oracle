use crate::monitor::SubscribeResponse;
use futures::{SinkExt, StreamExt};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json;
use std::fs;
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, oneshot};
use warp::ws;
use warp::{filters::BoxedFilter, Filter, Reply};

pub type DeviceId = u32;

#[derive(Debug, Serialize, Deserialize)]
pub struct Device {
    pub id: DeviceId,
    pub name: Option<String>,
    pub ipv4: Option<Ipv4Addr>,
}

impl Device {
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

#[derive(Debug, Clone)]
pub enum DeviceChange {
    Added(DeviceId),
    Removed(DeviceId),
}

pub struct Devices {
    pub list: Vec<Device>,
    pub changes: broadcast::Sender<DeviceChange>,
}

impl Devices {
    pub fn new_device_id(&self) -> DeviceId {
        // TODO: Race condition. Old Ids may still be referenced by tasks (and browsers)
        for i in 0..=(u32::MAX) {
            if self.list.iter().find(|d| d.id == i).is_none() {
                return i;
            }
        }
        panic!()
    }

    pub fn save_devices(&self) {
        fs::write(
            "data/devices.json",
            serde_json::to_string_pretty(&self.list).unwrap(),
        )
        .unwrap()
    }

    pub fn device_index(&self, id: DeviceId) -> Option<usize> {
        self.list
            .iter()
            .enumerate()
            .find(|d| d.1.id == id)
            .map(|d| d.0)
    }

    pub fn device(&self, id: DeviceId) -> &Device {
        &self.list[self.device_index(id).unwrap()]
    }
}

pub fn webserver(
    devices: Arc<Mutex<Devices>>,
    subscribe_status: mpsc::Sender<oneshot::Sender<SubscribeResponse>>,
) -> BoxedFilter<(impl Reply,)> {
    let devices_ = devices.clone();
    let list_devices = warp::path("devices")
        .and(warp::get())
        .and(warp::path::end())
        .map(move || {
            let devices = devices_.lock();
            serde_json::to_string(&devices.list).unwrap()
        });

    let devices_ = devices.clone();
    let add = warp::path("device")
        .and(warp::post())
        .and(warp::path::end())
        .and(warp::body::json())
        .map(move |mut device: Device| {
            //let ipv4 = from_map(&config, "ipv4");

            println!("device: {:?}", device);

            let mut devices = devices_.lock();
            device.id = devices.new_device_id();
            devices.changes.send(DeviceChange::Added(device.id)).ok();
            devices.list.push(device);
            devices.save_devices();

            ""
        });

    let devices_ = devices.clone();
    let remove = warp::path!("device" / u32)
        .and(warp::delete())
        .map(move |id: u32| {
            let mut devices = devices_.lock();

            let index = devices.device_index(id);
            index.map(|i| devices.list.remove(i));
            devices.save_devices();
            devices.changes.send(DeviceChange::Removed(id)).ok();

            ""
        });

    let status = warp::path!("devices" / "status")
        .and(warp::ws())
        .map(move |ws: ws::Ws| {
            let mut subscribe_status = subscribe_status.clone();
            ws.on_upgrade(|websocket| async move {
                let mut response = {
                    let (tx, rx) = oneshot::channel();
                    subscribe_status.send(tx).await.ok();
                    rx.await.unwrap()
                };

                let (mut tx, mut rx) = websocket.split();

                tx.send(ws::Message::text(
                    serde_json::to_string(&response.current).unwrap(),
                ))
                .await
                .ok(); // May fail due to the websocket closing

                loop {
                    tokio::select! {
                        Some(Ok(msg)) = rx.next() => {
                            if msg.is_close() {
                                break
                            }
                        },
                        Ok(msg) = response.receiver.recv() => {
                            tx.send(ws::Message::text(
                                serde_json::to_string(&[msg]).unwrap(),
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

pub fn load() -> Arc<Mutex<Devices>> {
    let list = serde_json::from_str(&fs::read_to_string("data/devices.json").unwrap()).unwrap();
    let (changes, _) = broadcast::channel(1000);
    Arc::new(Mutex::new(Devices { list, changes }))
}
