use crate::monitor::SubscribeResponse;
use crate::state::{Configuration, Device, State};
use futures::{FutureExt, SinkExt, StreamExt};
use serde_json;
use std::collections::HashMap;
use std::convert::TryInto;
use std::str;
use tokio::sync::{broadcast, mpsc, oneshot};
use warp::ws;
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

fn devices(
    state: &State,
    subscribe: mpsc::Sender<oneshot::Sender<SubscribeResponse>>,
) -> BoxedFilter<(impl Reply,)> {
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
        .map(move |mut device: Device| {
            //let ipv4 = from_map(&config, "ipv4");

            println!("device: {:?}", device);

            let mut state = state_.lock();
            device.id = state.new_device_id();
            state.devices.push(device);
            state.save_devices();

            ""
        });

    let state_ = state.clone();
    let remove = warp::path!("device" / u32)
        .and(warp::delete())
        .map(move |device: u32| {
            let mut state = state_.lock();

            let index = state
                .devices
                .iter()
                .enumerate()
                .find(|d| d.1.id == device)
                .map(|d| d.0);
            index.map(|i| state.devices.remove(i));
            state.save_devices();

            ""
        });

    let status = warp::path!("devices" / "status")
        .and(warp::ws())
        .map(move |ws: ws::Ws| {
            println!("websocket!");
            let mut subscribe = subscribe.clone();
            ws.on_upgrade(|websocket| async move {
                let mut response = {
                    let (tx, rx) = oneshot::channel();
                    subscribe.send(tx).await.ok();
                    rx.await.unwrap()
                };

                let (mut tx, mut rx) = websocket.split();

                tx.send(ws::Message::text(
                    serde_json::to_string(&response.current).unwrap(),
                ))
                .await
                .ok(); // May fail due to the websocket closing

                loop {
                    tokio::select! {
                        Some(Ok(msg)) = rx.next() => {
                            if msg.is_close() {
                                break
                            }
                        },
                        Ok(msg) = response.receiver.recv() => {
                            tx.send(ws::Message::text(
                                serde_json::to_string(&[msg]).unwrap(),
                            )).await.ok(); // May fail due to the websocket closing
                        },
                        else => {
                            break
                        }
                    };
                }
            })
        });

    devices.or(add).or(remove).or(status).boxed()
}

pub async fn webserver(
    state: &State,
    mut subscribe: mpsc::Sender<oneshot::Sender<SubscribeResponse>>,
) {
    let port = state.lock().config.web_port;

    let files = warp::fs::dir("web");
    let index = warp::fs::file("web/index.html");

    let app = files
        .or(index)
        .map(|reply| warp::reply::with_header(reply, "Cache-Control", "no-cache"));

    let api = warp::path("api").and(settings(state).or(devices(state, subscribe)));

    warp::serve(api.or(app)).run(([127, 0, 0, 1], port)).await;
}
