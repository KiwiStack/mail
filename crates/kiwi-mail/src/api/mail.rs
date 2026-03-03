use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use tracing::{error, instrument};

use kiwi_mail_types::{
    KiwiErrorBody, KiwiErrorResponse, KiwiResponse, MailDeleteResponse, MailFormat, MailMoveRequest,
    MailMoveResponse, MailReadQuery, MailSearchRequest, MailSendRequest, MailSendResponse,
    MailUpdateRequest, MailUpdateResponse, Mailbox, ResponseMeta, VacationResponse,
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
            req.sort_by.as_deref(),
            req.ascending,
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
        .email_send(
            &req.to,
            &req.subject,
            &req.body,
            &req.cc,
            &req.bcc,
            req.in_reply_to.as_deref(),
            req.references.as_deref(),
            &req.format,
        )
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

#[instrument(skip(state))]
pub async fn list_mailboxes(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Vec<Mailbox>> {
    let mailboxes = state
        .jmap
        .mailbox_list()
        .await
        .map_err(|e| {
            error!(error = %e, "mailbox list failed");
            api_error(StatusCode::BAD_GATEWAY, "upstream_error", &e.to_string())
        })?;

    Ok(Json(KiwiResponse {
        data: mailboxes,
        meta: meta(),
    }))
}

#[instrument(skip(state))]
pub async fn move_email(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<MailMoveRequest>,
) -> ApiResult<MailMoveResponse> {
    state
        .jmap
        .email_move(&id, &req.mailbox_id)
        .await
        .map_err(|e| {
            error!(error = %e, "mail move failed");
            api_error(StatusCode::BAD_GATEWAY, "upstream_error", &e.to_string())
        })?;

    Ok(Json(KiwiResponse {
        data: MailMoveResponse {
            id,
            mailbox_id: req.mailbox_id,
        },
        meta: meta(),
    }))
}

#[instrument(skip(state))]
pub async fn delete_email(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<MailDeleteResponse> {
    let status = state
        .jmap
        .email_delete(&id)
        .await
        .map_err(|e| {
            error!(error = %e, "mail delete failed");
            api_error(StatusCode::BAD_GATEWAY, "upstream_error", &e.to_string())
        })?;

    Ok(Json(KiwiResponse {
        data: MailDeleteResponse { id, status },
        meta: meta(),
    }))
}

#[instrument(skip(state))]
pub async fn update_email(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<MailUpdateRequest>,
) -> ApiResult<MailUpdateResponse> {
    state
        .jmap
        .email_update_keywords(&id, req.is_read, req.is_flagged)
        .await
        .map_err(|e| {
            error!(error = %e, "mail update failed");
            api_error(StatusCode::BAD_GATEWAY, "upstream_error", &e.to_string())
        })?;

    Ok(Json(KiwiResponse {
        data: MailUpdateResponse { id },
        meta: meta(),
    }))
}

#[instrument(skip(state))]
pub async fn download_attachment(
    State(state): State<Arc<AppState>>,
    Path((email_id, blob_id)): Path<(String, String)>,
) -> Result<(StatusCode, axum::http::HeaderMap, Vec<u8>), (StatusCode, Json<KiwiErrorResponse>)> {
    let _ = email_id; // Used for routing context; blob_id is sufficient for JMAP
    let bytes = state
        .jmap
        .attachment_download(&blob_id, "attachment")
        .await
        .map_err(|e| {
            error!(error = %e, "attachment download failed");
            api_error(StatusCode::BAD_GATEWAY, "upstream_error", &e.to_string())
        })?;

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        "application/octet-stream".parse().unwrap(),
    );
    headers.insert(
        axum::http::header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"attachment\"").parse().unwrap(),
    );

    Ok((StatusCode::OK, headers, bytes))
}

#[instrument(skip(state))]
pub async fn get_vacation(
    State(state): State<Arc<AppState>>,
) -> ApiResult<VacationResponse> {
    let vacation = state
        .jmap
        .vacation_get()
        .await
        .map_err(|e| {
            error!(error = %e, "vacation get failed");
            api_error(StatusCode::BAD_GATEWAY, "upstream_error", &e.to_string())
        })?;

    Ok(Json(KiwiResponse {
        data: vacation,
        meta: meta(),
    }))
}

#[instrument(skip(state))]
pub async fn set_vacation(
    State(state): State<Arc<AppState>>,
    Json(vacation): Json<VacationResponse>,
) -> ApiResult<VacationResponse> {
    state
        .jmap
        .vacation_set(&vacation)
        .await
        .map_err(|e| {
            error!(error = %e, "vacation set failed");
            api_error(StatusCode::BAD_GATEWAY, "upstream_error", &e.to_string())
        })?;

    Ok(Json(KiwiResponse {
        data: vacation,
        meta: meta(),
    }))
}
