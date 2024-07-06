use std::env::args;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get},
    Json, Router,
};
use mpd::Client;
use serde_json::{json, Value};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = Router::new().route("/", get(root));

    let listener = tokio::net::TcpListener::bind(args().last().unwrap())
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn root() -> Result<Json<Value>, AppError> {
    let mut conn = Client::connect("127.0.0.1:6600")?;

    let song = conn.currentsong()?;

    Ok(Json(json!({
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
