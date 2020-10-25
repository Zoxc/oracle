use std::sync::Arc;
use tokio::spawn;
use tokio::sync::mpsc;
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

    tokio::runtime::Builder::new()
        .basic_scheduler()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async move {
            let state = state::load();
            let log = Arc::new(log::Log::new());
            let devices = devices::load(state.clone());

            let notifier = spawn(notifier::notifier(devices.clone(), log.clone()));
            //let monitor = spawn(monitor::main_monitor(devices.clone(), rx, notify_tx));
            let web_server = spawn(webserver::webserver(
                devices.clone(),
                state.clone(),
                log.clone(),
            ));
            log.note("Server started up");
            let _ = tokio::join!(notifier /*, monitor*/, web_server);
        });
}
