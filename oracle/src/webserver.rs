use crate::devices::{self, Devices};
use crate::log::{self, Log};
use crate::state::{Config, State};
use serde_json;
use std::sync::Arc;
use std::{convert::Infallible, str};
use warp::{
    filters::BoxedFilter,
    header::headers_cloned,
    http::HeaderValue,
    hyper::{HeaderMap, StatusCode},
    reject::{self, Reject},
    reply, Filter, Rejection, Reply,
};

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

#[derive(Debug)]
struct Unauthorized;

impl Reject for Unauthorized {}

pub fn protected() -> impl Filter<Extract = (), Error = Rejection> + Clone {
    headers_cloned().and_then(authorize).untuple_one()
}

async fn authorize(headers: HeaderMap<HeaderValue>) -> Result<(), Rejection> {
    return Err(reject::custom(Unauthorized));
}

async fn api_error(err: Rejection) -> Result<impl Reply, Infallible> {
    if err.find::<Unauthorized>().is_some() {
        return Ok(reply::with_status(
            warp::reply::html("Unauthorized"),
            StatusCode::UNAUTHORIZED,
        ));
    }

    Ok(reply::with_status(
        warp::reply::html("Error"),
        StatusCode::BAD_REQUEST,
    ))
}

pub async fn webserver(devices: Arc<Devices>, state: State, log: Arc<Log>) {
    let port = state.lock().config.web_port;

    let files = warp::fs::dir("web");

    let app = files.map(|reply| warp::reply::with_header(reply, "Cache-Control", "no-cache"));

    let log = warp::path!("log").and(log::websocket(log));

    let protected_api = settings(&state).or(devices::webserver(devices)).or(log);

    let protected_api = protected().and(protected_api);

    let api = warp::path("api").and(protected_api.recover(api_error));

    warp::serve(api.or(app)).run(([127, 0, 0, 1], port)).await;
}
