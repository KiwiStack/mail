use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};

use crate::AppState;

mod health;
mod mail;
mod tools;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/healthz", get(health::healthz))
        .route("/tools", get(tools::list_tools))
        .route("/api/v1/mail/search", post(mail::search))
        .route("/api/v1/mail/{id}", get(mail::read))
        .route("/api/v1/mail/send", post(mail::send))
        .with_state(state)
}
