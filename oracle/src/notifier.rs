use crate::log::Log;
use crate::monitor::DeviceUpdate;
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

pub async fn notifier(state: State, log: Arc<Log>, mut receiver: mpsc::Receiver<DeviceUpdate>) {
    let mut buffer = Vec::new();
    let mut active = false;
    let (send_email_signal, mut email_signal) = mpsc::channel(10);

    loop {
        tokio::select! {
            Some(msg) = receiver.recv() => {
                buffer.push(msg);

                if !active {
                    active = true;
                    let mut send_email_signal = send_email_signal.clone();
                    spawn(async move {
                        delay_for(Duration::from_secs(30)).await;
                        send_email_signal.send(()).await.unwrap();
                    });
                }

            },
            Some(()) = email_signal.recv() => {
                log.note("Sending email");
                println!("sending email! {:#?}", buffer);
                buffer.clear();
                active = false;
            },
            else => { break }
        };
    }
}
