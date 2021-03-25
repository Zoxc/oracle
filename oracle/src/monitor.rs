use crate::devices::{Device, DeviceChange, DeviceId, Devices, ServiceStatus};
use crate::ping::Ping;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::SystemTime;
use std::{collections::HashMap, sync::atomic::AtomicBool};
use std::{net::Ipv4Addr, sync::atomic::Ordering};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::{delay_for, timeout, Duration};

#[derive(Debug, Clone)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        CancelToken(Arc::new(AtomicBool::new(false)))
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Eq, PartialEq)]
pub enum DeviceStatus {
    Unknown,
    Up,
    Down,
}

#[derive(Debug)]
struct DeviceState {
    status: DeviceStatus,
    since: SystemTime,
    ipv4: Ipv4Addr,
    aborted: Arc<AtomicBool>,
}

pub async fn device_monitor(
    devices: Arc<Devices>,
    device: Arc<Device>,
    ip: Ipv4Addr,
    cancel: CancelToken,
) {
    let id = device.conf.lock().id;
    let mut ping = devices.ping.clone();
    let mut notifier = devices.notifier.clone();
    let mut status = device.icmpv4.lock().status;

    loop {
        let new_status = match timeout(Duration::from_secs(1), ping.ping(ip)).await {
            Ok(_) => ServiceStatus::Up,
            Err(_) => {
                let mut new_status = ServiceStatus::Down;

                // Ping timed out, try 10 times before registering the device as down
                for _ in 0..10i32 {
                    delay_for(Duration::from_secs(1)).await;
                    match timeout(Duration::from_secs(1), ping.ping(ip)).await {
                        Ok(_) => {
                            new_status = ServiceStatus::Up;
                            break;
                        }
                        Err(_) => {}
                    }
                }

                new_status
            }
        };

        if cancel.cancelled() {
            break;
        }

        if status.map(|s| s.0) != Some(new_status) {
            let time = SystemTime::now();

            let new_status = Some((new_status, time));

            let change = {
                let mut icmpv4 = device.icmpv4.lock();

                // Check that we're not cancelled in the lock, so we have permission to update the device
                if cancel.cancelled() {
                    break;
                }

                icmpv4.status = new_status;

                let change = DeviceChange::IPv4Status {
                    device: id,
                    old: status,
                    new: new_status,
                };

                devices.changes.send(change.clone()).ok();

                change
            };

            notifier.send(change).await.unwrap();

            status = new_status;
        }

        delay_for(Duration::from_millis(10000)).await;
    }
}
/*
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
    devices: Arc<Devices>,
    mut subscribe_request: mpsc::Receiver<oneshot::Sender<SubscribeResponse>>,
    mut aborted: mpsc::Sender<DeviceUpdate>,
) {
    let ping = Ping::new();
    let start = SystemTime::now();

    let (tx, mut recv_monitor_msg) = mpsc::channel(1000);

    let (mut state, mut changes): (HashMap<DeviceId, DeviceState>, _) = {
        let devices = devices.lock();
        let state = devices
            .list
            .iter()
            .filter_map(|device| {
                device.ipv4.map(|ipv4| {
                    let aborted = Arc::new(AtomicBool::new(false));
                    tokio::spawn(device_monitor(
                        ipv4,
                        ping.clone(),
                        DeviceStatus::Unknown,
                        device.id,
                        tx.clone(),
                        aborted.clone(),
                    ));
                    (
                        device.id,
                        DeviceState {
                            status: DeviceStatus::Unknown,
                            since: start,
                            ipv4,
                            aborted,
                        },
                    )
                })
            })
            .collect();
        let changes = devices.changes.subscribe();
        (state, changes)
    };

    let (to_subscribers, _) = broadcast::channel(1000);

    loop {
        tokio::select! {
            Ok(change) = changes.recv() => {
                match change {
                    DeviceChange::Added(id) => {
                        if let Some(ipv4) = devices.lock().device(id).ipv4.clone() {
                            let aborted = Arc::new(AtomicBool::new(false));
                            tokio::spawn(device_monitor(
                                ipv4,
                                ping.clone(),
                                DeviceStatus::Unknown,
                                id,
                                tx.clone(),
                                aborted.clone(),
                            ));
                            state.insert(id, DeviceState {
                                status: DeviceStatus::Unknown,
                                since: SystemTime::now(),
                                ipv4,
                                aborted,
                            });
                        }
                    }
                    DeviceChange::Removed(id) => {
                        state.remove(&id).unwrap().aborted.store(true, Ordering::SeqCst);
                    }
                }
            },
            Some(msg) = recv_monitor_msg.recv() => {
                if let Some(mut state) = state.get_mut(&msg.id) {
                    if state.status != DeviceStatus::Unknown {
                        aborted.send(msg.clone()).await.unwrap();
                    }

                    state.status = msg.status;
                    state.since = msg.since;
                    to_subscribers.send(msg).ok(); // May fail due to no subscribers
                };
            },
            Some(subscribe_request) = subscribe_request.recv() => {
                subscribe_request.send(SubscribeResponse {
                    current: state.iter().map(|(id, state)| DeviceUpdate {
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
*/
