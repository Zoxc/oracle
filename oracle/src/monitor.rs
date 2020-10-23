use crate::state::{Configuration, Device, DeviceId, State};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::spawn;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::{delay_for, Duration};

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum DeviceStatus {
    Unknown,
    Up,
    Down,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct DeviceState {
    status: DeviceStatus,
    ipv4: Ipv4Addr,
}

async fn device_monitor(id: DeviceId, mut tx: mpsc::Sender<DeviceUpdate>) {
    loop {
        delay_for(Duration::from_millis(1000)).await;
        tx.send(DeviceUpdate {
            id,
            status: DeviceStatus::Up,
        })
        .await
        .unwrap();
        delay_for(Duration::from_millis(1000)).await;
        tx.send(DeviceUpdate {
            id,
            status: DeviceStatus::Down,
        })
        .await
        .unwrap();
        delay_for(Duration::from_millis(1000)).await;
        tx.send(DeviceUpdate {
            id,
            status: DeviceStatus::Unknown,
        })
        .await
        .unwrap();
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeviceUpdate {
    pub id: DeviceId,
    pub status: DeviceStatus,
}

#[derive(Debug)]
pub struct SubscribeResponse {
    pub current: Vec<DeviceUpdate>,
    pub receiver: broadcast::Receiver<DeviceUpdate>,
}

pub async fn main_monitor(
    state: State,
    mut subscribe_request: mpsc::Receiver<oneshot::Sender<SubscribeResponse>>,
) {
    let devices: HashMap<DeviceId, DeviceState> = {
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
                println!("msg: {:?}", msg);
                to_subscribers.send(msg).ok(); // May fail due to no subscribers
            },
            Some(subscribe_request) = subscribe_request.recv() => {
                subscribe_request.send(SubscribeResponse {
                    current: devices.iter().map(|(id, state)| DeviceUpdate {
                        id: *id,
                        status: state.status,
                    }).collect(),
                    receiver: to_subscribers.subscribe(),
                }).unwrap();
            },
            else => { break }
        };
    }
}
