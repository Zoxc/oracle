use log::Kind;
use std::{panic, sync::Arc};
use tokio::spawn;
use tracing_subscriber;

mod devices;
mod log;
mod monitor;
mod notifier;
mod ping;
mod state;
mod webserver;

fn main() {
    tracing_subscriber::fmt::init();

    let log = Arc::new(log::Log::new());

    let log_ = log.clone();
    panic::set_hook(Box::new(move |info| {
        let msg = match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &**s,
                None => "Box<Any>",
            },
        };

        log_.log(
            Kind::Error,
            &match info.location() {
                Some(location) => {
                    format!("Panic '{}' at {}:{}", msg, location.file(), location.line())
                }
                None => format!("Panic '{}'", msg),
            },
        );
    }));

    tokio::runtime::Builder::new()
        .basic_scheduler()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async move {
            let state = state::load();
            let devices = devices::load(state.clone(), log.clone());
            let web_server = spawn(webserver::webserver(
                devices.clone(),
                state.clone(),
                log.clone(),
            ));
            log.note("Server started up");
            web_server.await.unwrap();
        });
}
