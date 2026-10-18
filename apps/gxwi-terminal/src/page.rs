//! The page that draws the terminal, and the wire between it and here.
//!
//! The page is `page/`: `index.html`, `term.js`, `term.css`, and the
//! emulator it draws with, xterm.js, under `page/vendor/`. All of it is
//! built into this program and served from among the app's paths.
//!
//! THE WIRE is a websocket at `socket`, beside the page. Text is JSON and
//! says something about the terminal; anything else is the terminal's own
//! bytes, printed one way and typed the other.
//!
//!   process → page   { type: "stands", rows, cols }   the terminal afresh:
//!                                                     the page forgets what
//!                                                     it drew, and what
//!                                                     comes next prints the
//!                                                     terminal as it stands
//!                    { type: "size", rows, cols }     it is now this big
//!                    { type: "ended" }                what was running has
//!                                                     ended
//!                    bytes                            printed
//!   page → process   { type: "size", rows, cols }     what the page has
//!                                                     room for
//!                    bytes                            typed

use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::header::{CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, X_CONTENT_TYPE_OPTIONS};
use axum::response::IntoResponse;
use axum::routing::{any, get};
use serde_json::{Value, json};
use tokio::sync::broadcast::error::RecvError;

use crate::pty::Size;
use crate::terminal::{Said, Terminal};

/// Where the page is, among what the app serves.
pub const PATH: &str = "/term/";

/// The most a page may send at once: more than anything typed, and a great
/// deal pasted.
const MOST_TYPED: usize = 1024 * 1024;

/// What the page may do. Its scripts and styles are the app's own and
/// nothing else's, it speaks to nothing but the app, and the emulator sets
/// styles on what it draws.
const POLICY: &str = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; \
                      font-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'none'";

pub fn routes(terminal: Arc<Terminal>) -> Router {
    Router::new()
        .route(
            "/term/",
            file(
                "text/html; charset=utf-8",
                include_str!("../page/index.html"),
            ),
        )
        .route(
            "/term/term.js",
            file("text/javascript", include_str!("../page/term.js")),
        )
        .route(
            "/term/term.css",
            file("text/css", include_str!("../page/term.css")),
        )
        .route(
            "/term/vendor/xterm.mjs",
            file("text/javascript", include_str!("../page/vendor/xterm.mjs")),
        )
        .route(
            "/term/vendor/addon-fit.mjs",
            file(
                "text/javascript",
                include_str!("../page/vendor/addon-fit.mjs"),
            ),
        )
        .route(
            "/term/vendor/xterm.css",
            file("text/css", include_str!("../page/vendor/xterm.css")),
        )
        .route(
            "/term/socket",
            any(
                |State(terminal): State<Arc<Terminal>>, upgrade: WebSocketUpgrade| async move {
                    upgrade
                        .max_message_size(MOST_TYPED)
                        .on_upgrade(|socket| show(terminal, socket))
                },
            ),
        )
        .with_state(terminal)
}

fn file<S: Clone + Send + Sync + 'static>(
    kind: &'static str,
    body: &'static str,
) -> axum::routing::MethodRouter<S> {
    get(move || async move {
        (
            [
                (CONTENT_TYPE, kind),
                (X_CONTENT_TYPE_OPTIONS, "nosniff"),
                (CACHE_CONTROL, "no-cache"),
                (CONTENT_SECURITY_POLICY, POLICY),
            ],
            body,
        )
            .into_response()
    })
}

/// Shows one page the terminal, until the page goes.
async fn show(terminal: Arc<Terminal>, mut socket: WebSocket) {
    let text = |said: Value| Message::Text(said.to_string().into());
    'afresh: loop {
        let (size, stands, ended, mut said) = terminal.join();
        let first = [
            text(json!({ "type": "stands", "rows": size.rows, "cols": size.cols })),
            Message::Binary(stands.into()),
        ];
        for message in first {
            if socket.send(message).await.is_err() {
                return;
            }
        }
        if ended && socket.send(text(json!({ "type": "ended" }))).await.is_err() {
            return;
        }
        loop {
            tokio::select! {
                said = said.recv() => {
                    let message = match said {
                        Ok(Said::Printed(printed)) => Message::Binary(printed),
                        Ok(Said::Size(size)) => text(json!({ "type": "size", "rows": size.rows, "cols": size.cols })),
                        Ok(Said::Ended) => text(json!({ "type": "ended" })),
                        Ok(Said::Afresh) => continue 'afresh,
                        // Too far behind to be told what it missed, so it
                        // is shown the terminal as it now stands instead.
                        Err(RecvError::Lagged(_)) => continue 'afresh,
                        Err(RecvError::Closed) => return,
                    };
                    if socket.send(message).await.is_err() {
                        return;
                    }
                }
                heard = socket.recv() => match heard {
                    Some(Ok(Message::Binary(typed))) => terminal.type_in(typed.to_vec()),
                    Some(Ok(Message::Text(text))) => {
                        if let Some(size) = asked(text.as_str()) {
                            terminal.resize(size);
                        }
                    }
                    Some(Ok(_)) => {}
                    _ => return,
                },
            }
        }
    }
}

/// The size a page asked for, if that is what it said.
fn asked(text: &str) -> Option<Size> {
    let said: Value = serde_json::from_str(text).ok()?;
    if said["type"] != "size" {
        return None;
    }
    Some(Size::kept(said["rows"].as_u64()?, said["cols"].as_u64()?))
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
