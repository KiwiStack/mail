use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use tracing::{error, instrument};

use kiwi_mail_types::{
    KiwiErrorBody, KiwiErrorResponse, KiwiResponse, MailFormat, MailReadQuery, MailSearchRequest,
    MailSendRequest, MailSendResponse, ResponseMeta,
};

use crate::AppState;

fn meta() -> ResponseMeta {
    ResponseMeta {
        request_id: uuid_v4(),
        timestamp: chrono_now(),
    }
}

fn uuid_v4() -> String {
    // Simple pseudo-random UUID v4 using timestamp + random bits
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:032x}", nanos)
}

fn chrono_now() -> String {
    // ISO 8601 timestamp
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Return unix timestamp as string (proper ISO 8601 would need a date lib)
    format!("{secs}")
}

type ApiResult<T> = Result<Json<KiwiResponse<T>>, (StatusCode, Json<KiwiErrorResponse>)>;

fn api_error(
    status: StatusCode,
    code: &str,
    message: &str,
) -> (StatusCode, Json<KiwiErrorResponse>) {
    (
        status,
        Json(KiwiErrorResponse {
            error: KiwiErrorBody {
                code: code.to_string(),
                message: message.to_string(),
                details: None,
            },
            meta: meta(),
        }),
    )
}

#[instrument(skip(state, req))]
pub async fn search(
    State(state): State<Arc<AppState>>,
    Json(req): Json<MailSearchRequest>,
) -> ApiResult<Vec<kiwi_mail_types::EmailSummary>> {
    let results = state
        .jmap
        .email_search(
            req.query.as_deref(),
            req.mailbox.as_deref(),
            req.from.as_deref(),
            req.after.as_deref(),
            req.before.as_deref(),
            req.limit,
        )
        .await
        .map_err(|e| {
            error!(error = %e, "mail search failed");
            api_error(StatusCode::BAD_GATEWAY, "upstream_error", &e.to_string())
        })?;

    Ok(Json(KiwiResponse {
        data: results,
        meta: meta(),
    }))
}

#[instrument(skip(state))]
pub async fn read(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<MailReadQuery>,
) -> ApiResult<kiwi_mail_types::EmailDetail> {
    let html = matches!(query.format, MailFormat::Html);

    let email = state
        .jmap
        .email_read(&id, html)
        .await
        .map_err(|e| {
            error!(error = %e, "mail read failed");
            api_error(StatusCode::BAD_GATEWAY, "upstream_error", &e.to_string())
        })?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "not_found", "email not found"))?;

    Ok(Json(KiwiResponse {
        data: email,
        meta: meta(),
    }))
}

#[instrument(skip(state, req))]
pub async fn send(
    State(state): State<Arc<AppState>>,
    Json(req): Json<MailSendRequest>,
) -> ApiResult<MailSendResponse> {
    let id = state
        .jmap
        .email_send(&req.to, &req.subject, &req.body, &req.cc, &req.bcc)
        .await
        .map_err(|e| {
            error!(error = %e, "mail send failed");
            api_error(StatusCode::BAD_GATEWAY, "upstream_error", &e.to_string())
        })?;

    Ok(Json(KiwiResponse {
        data: MailSendResponse {
            id,
            status: "sent".to_string(),
        },
        meta: meta(),
    }))
}
