use crate::devices::{self, Devices};
use crate::log::{self, Log};
use crate::state::{Config, State};
use serde_json;
use std::str;
use std::sync::Arc;
use warp::{filters::BoxedFilter, Filter, Reply};

fn settings(state: &State) -> BoxedFilter<(impl Reply,)> {
    let state_ = state.clone();
    let read_settings = warp::path("settings")
        .and(warp::get())
        .and(warp::path::end())
        .map(move || {
            let config = state_.lock().config.clone();
            serde_json::to_string(&config).unwrap()
        });

    let state_ = state.clone();
    let write_settings = warp::path("settings")
        .and(warp::post())
        .and(warp::path::end())
        .and(warp::body::json())
        .map(move |config: Config| {
            if config.web_port != 0 && config.ping_interval != 0 {
                let mut state = state_.lock();
                state.config = config;
                state.save();
                ""
            } else {
                "error"
            }
        });

    read_settings.or(write_settings).boxed()
}

pub async fn webserver(devices: Arc<Devices>, state: State, log: Arc<Log>) {
    let port = state.lock().config.web_port;

    let files = warp::fs::dir("web");
    let index = warp::fs::file("web/index.html");

    let app = files
        .or(index)
        .map(|reply| warp::reply::with_header(reply, "Cache-Control", "no-cache"));

    let log = warp::path!("log").and(log::websocket(log));

    let api = settings(&state).or(devices::webserver(devices)).or(log);

    let api = warp::path("api").and(api);

    warp::serve(api.or(app)).run(([127, 0, 0, 1], port)).await;
}
