use crate::monitor::DeviceUpdate;
use crate::monitor::SubscribeResponse;
use crate::state::{Configuration, Device, DeviceId, State};
use futures::{FutureExt, SinkExt, StreamExt};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::convert::TryInto;
use std::fs;
use std::net::Ipv4Addr;
use std::str;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::spawn;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::{delay_for, Duration};
use warp::ws;
use warp::{filters::BoxedFilter, Filter, Reply};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Entry {
    pub kind: Kind,
    pub msg: String,
    pub time: SystemTime,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Kind {
    Note,
    Error,
}

pub struct Log {
    entries: Mutex<Vec<Entry>>,
    sender: broadcast::Sender<Entry>,
}

impl Log {
    pub fn new() -> Self {
        Log {
            entries: Mutex::new(Vec::new()),
            sender: broadcast::channel(1000).0,
        }
    }

    pub fn note(&self, val: &str) {
        let entry = Entry {
            kind: Kind::Note,
            msg: val.into(),
            time: SystemTime::now(),
        };
        let mut entries = self.entries.lock();
        entries.push(entry.clone());
        if entries.len() > 100 {
            entries.remove(0);
        }
        self.sender.send(entry).ok();
        eprintln!("[Note]: {}", val);
    }
}

pub fn websocket(log: Arc<Log>) -> BoxedFilter<(impl Reply,)> {
    warp::ws()
        .map(move |ws: ws::Ws| {
            let log = log.clone();
            ws.on_upgrade(|websocket| async move {
                let (entries, mut receiver) = {
                    let entries = log.entries.lock();
                    let entries = entries.clone();
                    (entries, log.sender.subscribe())
                };

                let (mut tx, mut rx) = websocket.split();

                tx.send(ws::Message::text(serde_json::to_string(&entries).unwrap()))
                    .await
                    .ok(); // May fail due to the websocket closing

                loop {
                    tokio::select! {
                        Some(Ok(msg)) = rx.next() => {
                            if msg.is_close() {
                                break
                            }
                        },
                        Ok(msg) = receiver.recv() => {
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
        })
        .boxed()
}
