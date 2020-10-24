use crate::devices::Devices;
use crate::log::Kind;
use crate::log::Log;
use crate::monitor::DeviceStatus;
use crate::monitor::DeviceUpdate;
use parking_lot::Mutex;
use std::sync::Arc;
use tokio::spawn;
use tokio::sync::mpsc;
use tokio::time::{delay_for, Duration};

pub async fn notifier(
    devices: Arc<Mutex<Devices>>,
    log: Arc<Log>,
    mut receiver: mpsc::Receiver<DeviceUpdate>,
) {
    let mut buffer = Vec::new();
    let mut active = false;
    let (send_email_signal, mut email_signal) = mpsc::channel(10);

    loop {
        tokio::select! {
            Some(msg) = receiver.recv() => {
                let desc = devices.lock().device(msg.id).desc();

                match msg.status {
                    DeviceStatus::Up => log.log(Kind::Note, &format!("Device {} is up", desc)),
                    DeviceStatus::Down => log.log(Kind::Error, &format!("Device {} is down", desc)),
                    _ => (),
                }

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
