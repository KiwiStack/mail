use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

use kiwi_mail_types::HealthResponse;

use crate::AppState;

pub async fn healthz(State(state): State<Arc<AppState>>) -> (StatusCode, Json<HealthResponse>) {
    let upstream_healthy = state.upstream.check_health().await;

    let (status_code, status_str, upstream_str) = if upstream_healthy {
        (StatusCode::OK, "ok", "healthy")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "degraded", "unhealthy")
    };

    (
        status_code,
        Json(HealthResponse {
            status: status_str.to_string(),
            upstream: upstream_str.to_string(),
        }),
    )
}
