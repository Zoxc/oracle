use crate::state::{Configuration, Device, State};
use serde_json;
use std::collections::HashMap;
use std::convert::TryInto;
use std::str;
use warp::{filters::BoxedFilter, Filter, Reply};

fn from_map<T: str::FromStr>(map: &HashMap<String, String>, field: &str) -> Option<T> {
    map.get(field).and_then(|data| str::parse(data.trim()).ok())
}

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
        .map(move |config: Configuration| {
            if config.web_port != 0 && config.ping_interval != 0 {
                let mut state = state_.lock();
                state.config = config;
                state.save_config();
                ""
            } else {
                "error"
            }
        });

    read_settings.or(write_settings).boxed()
}

fn devices(state: &State) -> BoxedFilter<(impl Reply,)> {
    let state_ = state.clone();
    let devices = warp::path("devices")
        .and(warp::get())
        .and(warp::path::end())
        .map(move || {
            let state = state_.lock();
            serde_json::to_string(&state.devices).unwrap()
        });

    let state_ = state.clone();
    let add = warp::path("device")
        .and(warp::post())
        .and(warp::path::end())
        .and(warp::body::json())
        .map(move |config: HashMap<String, String>| {
            let ipv4 = from_map(&config, "ipv4");

            println!("ip: {:?}", ipv4);

            ipv4.map(|ipv4| {
                let mut state = state_.lock();
                let id = state.devices.len().try_into().unwrap();
                state.devices.push(Device { id, ipv4 });
                state.save_devices();
            });

            ""
        });

    devices.or(add).boxed()
}

pub fn webserver(state: &State) {
    let port = state.lock().config.web_port;

    let files = warp::path("static").and(warp::fs::dir("web"));
    let index = warp::fs::file("web/index.html");

    let app = files
        .or(index)
        .map(|reply| warp::reply::with_header(reply, "Cache-Control", "no-cache"));

    let api = warp::path("api").and(settings(state).or(devices(state)));

    let server = async move {
        warp::serve(api.or(app)).run(([127, 0, 0, 1], port)).await;
    };

    tokio::runtime::Builder::new()
        .basic_scheduler()
        .enable_all()
        .build()
        .unwrap()
        .block_on(server);
}
