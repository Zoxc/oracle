use warp::Filter;

pub fn webserver() {
    tokio::runtime::Builder::new()
        .basic_scheduler()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let files = warp::path("static").and(warp::fs::dir("web"));
            //let index = warp::path::end().and(warp::fs::file("web/index.html"));
            //let static_files = warp::get().and(files.or(index));
            let index = warp::fs::file("web/index.html");

            let app = files
                .or(index)
                .map(|reply| warp::reply::with_header(reply, "Cache-Control", "no-cache"));

            let settings = warp::path("settings")
                .and(warp::path::end())
                .map(|| format!("{{ \"port\": 56 }}"));

            let api = warp::path("api").and(settings);

            warp::serve(api.or(app)).run(([127, 0, 0, 1], 3030)).await;
        })
}
