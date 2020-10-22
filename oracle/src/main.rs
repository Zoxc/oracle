mod ping;
mod state;
mod webserver;

fn main() {
    let state = state::load();
    webserver::webserver(&state);
}
