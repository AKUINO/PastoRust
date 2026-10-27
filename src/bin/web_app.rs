use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::header,
    response::{Html, IntoResponse},
    routing::get,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[path = "../web.rs"]
mod web;

#[derive(Clone, Deserialize, Serialize)]
struct AppState {
    mode: &'static str,
    gpio15: bool,
    adc26: u16,
}

#[derive(Deserialize)]
struct SimulatedInputs {
    gpio15: bool,
    adc26: u16,
}

async fn index() -> Html<&'static str> {
    Html(std::str::from_utf8(web::INDEX_HTML).expect("index.html must be valid UTF-8"))
}

async fn stylesheet() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        web::STYLE_CSS,
    )
}

async fn javascript() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        web::APP_JS,
    )
}

async fn get_state(State(state): State<Arc<RwLock<AppState>>>) -> Json<AppState> {
    Json(state.read().await.clone())
}

async fn set_state(
    State(state): State<Arc<RwLock<AppState>>>,
    Json(inputs): Json<SimulatedInputs>,
) -> Json<AppState> {
    let mut state = state.write().await;
    state.gpio15 = inputs.gpio15;
    state.adc26 = inputs.adc26.min(4095);
    Json(state.clone())
}

#[tokio::main]
async fn main() {
    let state = Arc::new(RwLock::new(AppState {
        mode: web::MODE,
        gpio15: false,
        adc26: 2048,
    }));

    let app = Router::new()
        .route("/", get(index))
        .route("/style.css", get(stylesheet))
        .route("/app.js", get(javascript))
        .route("/api/state", get(get_state).post(set_state))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8000")
        .await
        .expect("could not bind local application to 127.0.0.1:8000");
    println!(
        "PastoRust (mode simulation): http://{}",
        listener.local_addr().unwrap()
    );
    axum::serve(listener, app)
        .await
        .expect("local web server stopped unexpectedly");
}
