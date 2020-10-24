use std::sync::Arc;
use tokio::spawn;
use tokio::sync::mpsc;

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
            let log = Arc::new(log::Log::new());
            let (notify_tx, notify_rx) = mpsc::channel(1000);

            let (tx, rx) = mpsc::channel(1000);

            let notifier = spawn(notifier::notifier(state.clone(), log.clone(), notify_rx));
            let monitor = spawn(monitor::main_monitor(state.clone(), rx, notify_tx));
            let web_server = spawn(webserver::webserver(state.clone(), tx, log.clone()));
            log.note("Server started up");
            let _ = tokio::join!(notifier, monitor, web_server);
        });
}
