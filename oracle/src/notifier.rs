use crate::devices::Devices;
use crate::devices::ServiceStatus;
use crate::log::Kind;
use crate::log::Log;
use crate::{
    devices::{DeviceChange, DeviceId},
    state::Conf,
};
use chrono::{DateTime, Utc};
use lettre::{smtp::authentication::Credentials, SmtpClient, Transport};
use lettre_email::EmailBuilder;
use std::{sync::Arc, time::SystemTime};
use tokio::sync::mpsc;
use tokio::time::{delay_for, Duration};
use tokio::{spawn, task};

pub fn send_email(
    devices: &Arc<Devices>,
    log: &Arc<Log>,
    conf: &Conf,
    email_receiver: &str,
    changes: Vec<(DeviceId, (ServiceStatus, SystemTime))>,
) -> bool {
    let mut body = "The following network changes were detected:\n\n".to_owned();

    for change in changes {
        let time: DateTime<Utc> = change.1 .1.into();
        let time = time.to_rfc2822();
        let verb = match change.1 .0 {
            ServiceStatus::Up => "up",
            ServiceStatus::Down => "down",
        };
        let desc = devices.device(change.0).conf.lock().desc();
        body.push_str(&format!(" - {} went {} at {:?}", desc, verb, time));
    }

    let smtp = conf.lock().smtp.clone().unwrap();
    let email = EmailBuilder::new()
        .from(smtp.from)
        .to(email_receiver)
        .subject("Network changes")
        .body(body)
        .build();

    let email = match email {
        Ok(email) => email,
        _ => {
            log.log(Kind::Error, "Unable to create email");
            return false;
        }
    };

    let creds = Credentials::new(smtp.user, smtp.password);

    let client = match SmtpClient::new_simple(&smtp.server) {
        Ok(client) => client,
        _ => {
            log.log(Kind::Error, "Unable to create SMTP client");
            return false;
        }
    };

    let mut client = client.credentials(creds).transport();

    match client.send(email.into()) {
        Ok(_) => {
            log.note(&format!("Sent email to {}", email_receiver));
            true
        }
        Err(_) => {
            log.log(
                Kind::Error,
                &format!("Unable to send email to {}", email_receiver),
            );
            false
        }
    }
}

pub async fn notifier(
    conf: Conf,
    devices: Arc<Devices>,
    log: Arc<Log>,
    email_receiver: String,
    mut receiver: mpsc::Receiver<DeviceChange>,
) {
    let mut buffer = Vec::new();
    let mut active = false;
    let (send_email_signal, mut email_signal) = mpsc::channel(10);

    log.note(&format!("Notifier for {} starting", email_receiver));

    loop {
        tokio::select! {
            Some(change) = receiver.recv() => {
                let (device, status) = match change {
                    DeviceChange::IPv4Status { device, old: Some(_), new: Some(new) } => {
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
                let result ={
                    let buffer = buffer.clone();
                    let log = log.clone();
                    let conf = conf.clone();
                    let devices = devices.clone();
                    let email_receiver = email_receiver.clone();

                    task::spawn_blocking(move || send_email(&devices, &log, &conf, &email_receiver, buffer)).await.unwrap()
                };

                if result {
                    buffer.clear();
                } else {
                    // Try again in 5 mins
                    let mut send_email_signal = send_email_signal.clone();
                    spawn(async move {
                        delay_for(Duration::from_secs(300)).await;
                        send_email_signal.send(()).await.unwrap();
                    });
                }

                active = false;
            },
            else => { break }
        };
    }
}
