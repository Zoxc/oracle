use tokio::spawn;
use tokio::sync::mpsc;

mod monitor;
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
            let (tx, rx) = mpsc::channel(1000);

            spawn(monitor::main_monitor(state.clone(), rx));
            webserver::webserver(&state, tx).await;
        });
}
