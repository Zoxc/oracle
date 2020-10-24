use std::sync::Arc;
use tokio::spawn;
use tokio::sync::mpsc;

mod devices;
mod log;
mod monitor;
mod notifier;
mod ping;
mod state;
mod webserver;

fn main() {
    let state = state::load();

    tokio::runtime::Builder::new()
        .basic_scheduler()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async move {
            let devices = devices::load();

            let log = Arc::new(log::Log::new());
            let (notify_tx, notify_rx) = mpsc::channel(1000);

            let (tx, rx) = mpsc::channel(1000);

            let notifier = spawn(notifier::notifier(devices.clone(), log.clone(), notify_rx));
            let monitor = spawn(monitor::main_monitor(devices.clone(), rx, notify_tx));
            let web_server = spawn(webserver::webserver(
                devices.clone(),
                state.clone(),
                tx,
                log.clone(),
            ));
            log.note("Server started up");
            let _ = tokio::join!(notifier, monitor, web_server);
        });
}
