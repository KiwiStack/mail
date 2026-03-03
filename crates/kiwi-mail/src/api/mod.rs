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
        .route("/api/v1/mailboxes", get(mail::list_mailboxes))
        .route("/api/v1/mail/search", post(mail::search))
        .route("/api/v1/mail/vacation", get(mail::get_vacation).put(mail::set_vacation))
        .route("/api/v1/mail/send", post(mail::send))
        .route("/api/v1/mail/{id}", get(mail::read).delete(mail::delete_email).patch(mail::update_email))
        .route("/api/v1/mail/{id}/move", post(mail::move_email))
        .route("/api/v1/mail/{id}/attachments/{blob_id}", get(mail::download_attachment))
        .with_state(state)
}
