use crate::log::{self, Log};
use crate::state::{Config, State};
use crate::{
    devices::{self, Devices},
    state::User,
};
use parking_lot::Mutex;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use serde_json::{self, json};
use std::sync::Arc;
use std::{convert::Infallible, str, time::Instant};
use warp::{
    cookie,
    filters::BoxedFilter,
    hyper::StatusCode,
    reject::{self, Reject},
    reply::{self, Response},
    Filter, Rejection, Reply,
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

pub fn protected(
    sessions: Arc<Mutex<Vec<(String, Instant)>>>,
) -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::any()
        .map(move || sessions.clone())
        .and(cookie::optional("token"))
        .and_then(authorize)
        .untuple_one()
}

async fn authorize(
    sessions: Arc<Mutex<Vec<(String, Instant)>>>,
    cookie: Option<String>,
) -> Result<(), Rejection> {
    let ok = {
        let now = Instant::now();
        let mut sessions = sessions.lock();
        sessions.retain(|session| session.1.saturating_duration_since(now).as_secs() < 2592000);

        sessions
            .iter()
            .find(|session| Some(&session.0) == cookie.as_ref())
            .is_some()
    };

    if ok {
        Ok(())
    } else {
        Err(reject::custom(Unauthorized))
    }
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

fn login(state: State, sessions: Arc<Mutex<Vec<(String, Instant)>>>, user: User) -> Response {
    let found = state.lock().users.contains(&user);

    if found {
        let cookie: String = thread_rng().sample_iter(&Alphanumeric).take(128).collect();

        sessions.lock().push((cookie.clone(), Instant::now()));

        let val = json!({ "result": "ok" });
        reply::with_header(
            reply::json(&val),
            "Set-Cookie",
            format!(
                "token={}; HttpOnly; Max-Age=2592000; SameSite=Strict",
                cookie
            ),
        )
        .into_response()
    } else {
        let val = json!({ "result": "error" });
        reply::json(&val).into_response()
    }
}

pub async fn webserver(devices: Arc<Devices>, state: State, log: Arc<Log>) {
    let port = state.lock().config.web_port;

    let files = warp::fs::dir("web");

    let app = files.map(|reply| warp::reply::with_header(reply, "Cache-Control", "no-cache"));

    let log = warp::path!("log").and(log::websocket(log));

    let sessions = Arc::new(Mutex::new(Vec::new()));

    let state_ = state.clone();
    let sessions_ = sessions.clone();
    let login = warp::path!("login")
        .and(warp::any().map(move || state_.clone()))
        .and(warp::any().map(move || sessions_.clone()))
        .and(warp::body::json())
        .map(login);

    let logout = warp::path!("logout").map(|| {
        reply::with_header(
            reply::html(""),
            "Set-Cookie",
            "token=; HttpOnly; Max-Age=0; SameSite=Strict",
        )
    });

    let protected_api = settings(&state).or(devices::webserver(devices)).or(log);

    let protected_api = protected(sessions).and(protected_api);

    let api = login.or(logout).or(protected_api);

    let api = warp::path("api").and(api.recover(api_error));

    warp::serve(api.or(app)).run(([0, 0, 0, 0], port)).await;
}
