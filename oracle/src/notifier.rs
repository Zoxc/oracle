use crate::devices::Devices;
use crate::devices::ServiceStatus;
use crate::log::Kind;
use crate::log::Log;
use crate::{
    devices::{DeviceChange, DeviceId},
    state::Conf,
};
use chrono::{DateTime, Local};
use lettre::{
    smtp::authentication::Credentials, ClientSecurity, ClientTlsParameters, SmtpClient, Transport,
};
use lettre_email::{EmailBuilder, Mailbox};
use native_tls::{Protocol, TlsConnector};
use std::{
    sync::Arc,
    time::{Instant, SystemTime},
};
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
        let time: DateTime<Local> = change.1 .1.into();
        let time = time.to_rfc2822();
        let verb = match change.1 .0 {
            ServiceStatus::Up => "up",
            ServiceStatus::Down => "down",
        };
        let desc = devices.device(change.0).conf.lock().desc();
        body.push_str(&format!(" - Device `{}` went {} at {}\n", desc, verb, time));
    }

    let smtp = conf.lock().smtp.clone().unwrap();

    let from = match smtp.from.parse::<Mailbox>() {
        Ok(from) => from,
        _ => {
            log.log(
                Kind::Error,
                &format!("Unable to parse {} as an email address", smtp.from),
            );
            return false;
        }
    };

    let to = match email_receiver.parse::<Mailbox>() {
        Ok(to) => to,
        _ => {
            log.log(
                Kind::Error,
                &format!("Unable to parse {} as an email address", email_receiver),
            );
            return false;
        }
    };

    let email = EmailBuilder::new()
        .from(from)
        .to(to)
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

    let mut tls_builder = TlsConnector::builder();
    tls_builder.min_protocol_version(Some(Protocol::Tlsv12));

    let tls_parameters =
        ClientTlsParameters::new(smtp.server.clone(), tls_builder.build().unwrap());

    let client = match SmtpClient::new(
        (smtp.server.clone(), 587),
        ClientSecurity::Required(tls_parameters),
    ) {
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
        Err(error) => {
            log.log(
                Kind::Error,
                &format!("Unable to send email to {}\n{}", email_receiver, error),
            );
            false
        }
    }
}

pub async fn generate_email_signal(
    devices: Arc<Devices>,
    mut send_email_signal: mpsc::Sender<()>,
    secs: u64,
) {
    delay_for(Duration::from_secs(secs)).await;

    // Acquire the token to send emails
    loop {
        {
            let mut lock = devices.last_email.lock();

            match *lock {
                Some(last) => {
                    if Instant::now().saturating_duration_since(last).as_secs() > 30 {
                        *lock = None;
                        break;
                    }
                }
                None => (),
            }
        };

        delay_for(Duration::from_secs(35)).await;
    }

    send_email_signal.send(()).await.unwrap();
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

                buffer.push((device, status));

                if !active {
                    active = true;
                    spawn(generate_email_signal(devices.clone(), send_email_signal.clone(), 30));
                }
            },
            Some(()) = email_signal.recv() => {
                let result = {
                    let buffer = buffer.clone();
                    let log = log.clone();
                    let conf = conf.clone();
                    let devices = devices.clone();
                    let email_receiver = email_receiver.clone();

                    task::spawn_blocking(move || send_email(&devices, &log, &conf, &email_receiver, buffer)).await.unwrap()

                };

                // Return email token
                *devices.last_email.lock() = Some(Instant::now());

                if result {
                    buffer.clear();
                    active = false;
                } else {
                    // Try again in 5 mins
                    spawn(generate_email_signal(devices.clone(), send_email_signal.clone(), 300));
                }

            },
            else => { break }
        };
    }
}
