use serde::{Deserialize, Serialize};

// --- Envelope types ---

#[derive(Debug, Serialize, Deserialize)]
pub struct KiwiResponse<T> {
    pub data: T,
    pub meta: ResponseMeta,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseMeta {
    pub request_id: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KiwiErrorResponse {
    pub error: KiwiErrorBody,
    pub meta: ResponseMeta,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KiwiErrorBody {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

// --- Address ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub email: String,
}

// --- Mail search ---

#[derive(Debug, Serialize, Deserialize)]
pub struct MailSearchRequest {
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub mailbox: Option<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub after: Option<String>,
    #[serde(default)]
    pub before: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    20
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmailSummary {
    pub id: String,
    pub from: Vec<Address>,
    pub to: Vec<Address>,
    pub subject: String,
    pub received_at: String,
    pub preview: String,
    #[serde(default)]
    pub is_read: bool,
    #[serde(default)]
    pub is_flagged: bool,
}

// --- Mail read ---

#[derive(Debug, Deserialize)]
pub struct MailReadQuery {
    #[serde(default = "default_format")]
    pub format: MailFormat,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MailFormat {
    #[default]
    Text,
    Html,
}

fn default_format() -> MailFormat {
    MailFormat::Text
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmailDetail {
    pub id: String,
    pub from: Vec<Address>,
    pub to: Vec<Address>,
    #[serde(default)]
    pub cc: Vec<Address>,
    pub subject: String,
    pub received_at: String,
    pub body: String,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub message_id: Option<String>,
    #[serde(default)]
    pub in_reply_to: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Attachment {
    pub name: String,
    pub size: u64,
    #[serde(rename = "type")]
    pub content_type: String,
}

// --- Mail send ---

#[derive(Debug, Serialize, Deserialize)]
pub struct MailSendRequest {
    pub to: Vec<String>,
    pub subject: String,
    pub body: String,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub bcc: Vec<String>,
    #[serde(default)]
    pub in_reply_to: Option<String>,
    #[serde(default)]
    pub references: Option<String>,
    #[serde(default = "default_send_format")]
    pub format: SendFormat,
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SendFormat {
    #[default]
    Text,
    Html,
}

fn default_send_format() -> SendFormat {
    SendFormat::Text
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MailSendResponse {
    pub id: String,
    pub status: String,
}

// --- Mailbox ---

#[derive(Debug, Serialize, Deserialize)]
pub struct Mailbox {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub total_emails: u64,
    #[serde(default)]
    pub unread_emails: u64,
}

// --- Mail move ---

#[derive(Debug, Serialize, Deserialize)]
pub struct MailMoveRequest {
    pub mailbox_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MailMoveResponse {
    pub id: String,
    pub mailbox_id: String,
}

// --- Mail update ---

#[derive(Debug, Serialize, Deserialize)]
pub struct MailUpdateRequest {
    #[serde(default)]
    pub is_read: Option<bool>,
    #[serde(default)]
    pub is_flagged: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MailUpdateResponse {
    pub id: String,
}

// --- Mail delete ---

#[derive(Debug, Serialize, Deserialize)]
pub struct MailDeleteResponse {
    pub id: String,
    pub status: String,
}

// --- Health ---

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub upstream: String,
}

// --- MCP Tools ---

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}
