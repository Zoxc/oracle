use crate::devices::DeviceChange;
use crate::devices::Devices;
use crate::devices::ServiceStatus;
use crate::log::Kind;
use crate::log::Log;
use crate::monitor::DeviceStatus;
use parking_lot::Mutex;
use std::sync::Arc;
use tokio::spawn;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{delay_for, Duration};

pub async fn notifier(
    devices: Arc<Devices>,
    log: Arc<Log>,
    mut receiver: mpsc::Receiver<DeviceChange>,
) {
    let mut buffer = Vec::new();
    let mut active = false;
    let (send_email_signal, mut email_signal) = mpsc::channel(10);

    loop {
        tokio::select! {
            Some(change) = receiver.recv() => {
                let (device, status) = match change {
                    DeviceChange::IPv4Status { device, old: Some(old), new: Some(new) } => {
                        (device, new)
                    }
                    _ => continue,
                };

                let desc = devices.device(device).conf.lock().desc();

                match status.0 {
                    ServiceStatus::Up => log.log(Kind::Note, &format!("Device {} is up", desc)),
                    ServiceStatus::Down => log.log(Kind::Error, &format!("Device {} is down", desc)),
                }

                buffer.push((device, status));

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
