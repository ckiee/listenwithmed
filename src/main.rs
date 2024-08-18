use std::{
    collections::HashMap,
    env::args,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{anyhow, Context};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use clap::Parser;
use mpd::Client;
use nid::Nanoid;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::time::Instant;

/// a http server that replies w mpd status
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    listen_address: String,
}

#[derive(Debug)]
struct AppState {
    listeners: HashMap<Nanoid, Instant>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    tracing_subscriber::fmt::init();

    let app_state = Arc::new(Mutex::new(AppState {
        listeners: HashMap::new(),
    }));
    let app = Router::new().route("/", get(root)).with_state(app_state);

    let listener = tokio::net::TcpListener::bind(args.listen_address)
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Debug, Deserialize, Clone, Copy)]
struct RootQuery {
    listener_id: Nanoid,
}

async fn root(
    query: Option<Query<RootQuery>>,
    State(state): State<Arc<Mutex<AppState>>>,
) -> Result<Json<Value>, AppError> {
    let listeners = {
        let mut state = state
            .lock()
            .map_err(|_| anyhow!("unlocking state for .listeners"))?;

        if let Some(query) = query {
            println!("insert");
            state.listeners.insert(query.listener_id, Instant::now());
        }

        state.listeners = state
            .listeners
            .clone() // meh but rustc optimizes so ok
            .into_iter()
            .filter(|(_, then)| then.elapsed() < Duration::from_secs(5))
            .collect();
        state.listeners.iter().count()
    };

    let mut conn = Client::connect("127.0.0.1:6600")?;
    let song = conn.currentsong()?;

    // TODO
    // let rating = song.map(|s| conn.sticker("song", &s.file, "rating"));

    let output = conn
        .outputs()?
        .into_iter()
        .find(|f| f.name == "listenwithme")
        .unwrap();

    if !output.enabled {
        return Ok(Json(json!({
            "available": false
        })));
    }

    Ok(Json(json!({
        "available": true,
        "listeners": listeners,
        "status": conn.status()?,
        "playing": {
            "song": song,
            // FIXME: bwah, has .unwrap
            "comments": song.map(|s| conn.readcomments(&s).unwrap()
                                .filter(|r| r.is_ok())
                                .map(|r| r.unwrap())
                                .collect::<Vec<(String, String)>>())
        },
    })))
}

// blabla https://github.com/tokio-rs/axum/blob/905a1a72a31ffe1004acd080115f132a4dac56f7/examples/anyhow-error-response/src/main.rs
struct AppError(anyhow::Error);
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
