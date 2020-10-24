use crate::state::{Configuration, Device, DeviceId, State};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::spawn;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::{delay_for, Duration};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Eq, PartialEq)]
pub enum DeviceStatus {
    Unknown,
    Up,
    Down,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct DeviceState {
    status: DeviceStatus,
    since: SystemTime,
    ipv4: Ipv4Addr,
}

async fn device_monitor(id: DeviceId, mut tx: mpsc::Sender<DeviceUpdate>) {
    loop {
        delay_for(Duration::from_millis(10000)).await;
        tx.send(DeviceUpdate {
            id,
            status: DeviceStatus::Up,
            since: SystemTime::now(),
        })
        .await
        .unwrap();
        delay_for(Duration::from_millis(10000)).await;
        tx.send(DeviceUpdate {
            id,
            status: DeviceStatus::Down,
            since: SystemTime::now(),
        })
        .await
        .unwrap();
        delay_for(Duration::from_millis(10000)).await;
        tx.send(DeviceUpdate {
            id,
            status: DeviceStatus::Unknown,
            since: SystemTime::now(),
        })
        .await
        .unwrap();
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeviceUpdate {
    pub id: DeviceId,
    pub status: DeviceStatus,
    pub since: SystemTime,
}

#[derive(Debug)]
pub struct SubscribeResponse {
    pub current: Vec<DeviceUpdate>,
    pub receiver: broadcast::Receiver<DeviceUpdate>,
}

pub async fn main_monitor(
    state: State,
    mut subscribe_request: mpsc::Receiver<oneshot::Sender<SubscribeResponse>>,
    mut notify: mpsc::Sender<DeviceUpdate>,
) {
    let start = SystemTime::now();
    let mut devices: HashMap<DeviceId, DeviceState> = {
        let state = state.lock();
        state
            .devices
            .iter()
            .filter_map(|device| {
                device.ipv4.map(|ipv4| {
                    (
                        device.id,
                        DeviceState {
                            status: DeviceStatus::Unknown,
                            since: start,
                            ipv4,
                        },
                    )
                })
            })
            .collect()
    };

    let (to_subscribers, _) = broadcast::channel(1000);

    let (tx, mut recv_monitor_msg) = mpsc::channel(1000);

    for (&id, state) in devices.iter() {
        tokio::spawn(device_monitor(id, tx.clone()));
    }

    loop {
        tokio::select! {
            Some(msg) = recv_monitor_msg.recv() => {
                let mut state = devices.get_mut(&msg.id).unwrap();

                if state.status != DeviceStatus::Unknown {
                    notify.send(msg.clone()).await.unwrap();
                }

                state.status = msg.status;
                state.since = msg.since;
                to_subscribers.send(msg).ok(); // May fail due to no subscribers
            },
            Some(subscribe_request) = subscribe_request.recv() => {
                subscribe_request.send(SubscribeResponse {
                    current: devices.iter().map(|(id, state)| DeviceUpdate {
                        id: *id,
                        status: state.status,
                        since: state.since,
                    }).collect(),
                    receiver: to_subscribers.subscribe(),
                }).unwrap();
            },
            else => { break }
        };
    }
}
