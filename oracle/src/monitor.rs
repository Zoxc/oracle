use crate::ping::Ping;
use crate::state::{DeviceId, State};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::time::SystemTime;
use tokio::sync::{broadcast, mpsc, oneshot, Notify};
use tokio::time::{delay_for, timeout, Duration};

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

async fn device_monitor(
    state: State,
    mut ping: Ping,
    mut status: DeviceStatus,
    id: DeviceId,
    mut tx: mpsc::Sender<DeviceUpdate>,
    mut notify: Notify,
) {
    let ip = if let Some(ip) = state.lock().device(id).ipv4.clone() {
        ip
    } else {
        return;
    };

    loop {
        let new_status = match timeout(Duration::from_secs(1), ping.ping(ip)).await {
            Ok(_) => DeviceStatus::Up,
            Err(_) => {
                let mut new_status = DeviceStatus::Down;

                // Ping timed out, try 10 times before registering the device as down
                for _ in 0..9 {
                    delay_for(Duration::from_secs(1)).await;
                    match timeout(Duration::from_secs(1), ping.ping(ip)).await {
                        Ok(_) => {
                            new_status = DeviceStatus::Up;
                            break;
                        }
                        Err(_) => {}
                    }
                }

                new_status
            }
        };

        if status != new_status {
            status = new_status;
            tx.send(DeviceUpdate {
                id,
                status,
                since: SystemTime::now(),
            })
            .await
            .unwrap();
        }

        delay_for(Duration::from_millis(10000)).await;
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
    let ping = Ping::new();
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

    for (&id, device_state) in devices.iter() {
        tokio::spawn(device_monitor(
            state.clone(),
            ping.clone(),
            device_state.status,
            id,
            tx.clone(),
        ));
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
