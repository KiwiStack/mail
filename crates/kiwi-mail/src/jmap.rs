use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{debug, instrument};

use kiwi_mail_types::{Address, Attachment, EmailDetail, EmailSummary, Mailbox, SendFormat};

#[derive(Debug, thiserror::Error)]
pub enum JmapError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("JMAP error: {0}")]
    Protocol(String),
    #[error("unexpected response shape: {0}")]
    UnexpectedResponse(String),
}

type Result<T> = std::result::Result<T, JmapError>;

#[derive(Debug, Deserialize)]
struct JmapSession {
    #[serde(rename = "primaryAccounts")]
    primary_accounts: std::collections::HashMap<String, String>,
}

pub struct JmapClient {
    http: reqwest::Client,
    endpoint: String,
    account_id: String,
    drafts_mailbox_id: Option<String>,
    trash_mailbox_id: Option<String>,
    admin_user: String,
    admin_pass: String,
}

#[derive(Debug, Serialize)]
struct JmapRequest {
    using: Vec<&'static str>,
    #[serde(rename = "methodCalls")]
    method_calls: Vec<(String, Value, String)>,
}

#[derive(Debug, Deserialize)]
struct JmapResponse {
    #[serde(rename = "methodResponses")]
    method_responses: Vec<(String, Value, String)>,
}

impl JmapClient {
    pub async fn discover(upstream_addr: &str, admin_user: &str, admin_pass: &str) -> Result<Self> {
        let http = reqwest::Client::new();

        let session_url = format!("{upstream_addr}/.well-known/jmap");
        let session: JmapSession = http
            .get(&session_url)
            .basic_auth(admin_user, Some(admin_pass))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let account_id = session
            .primary_accounts
            .get("urn:ietf:params:jmap:mail")
            .or_else(|| session.primary_accounts.values().next())
            .cloned()
            .ok_or_else(|| JmapError::UnexpectedResponse("no accounts in session".into()))?;

        debug!(%account_id, "JMAP session discovered");

        let endpoint = format!("{upstream_addr}/jmap");

        let mut client = Self {
            http,
            endpoint,
            account_id,
            drafts_mailbox_id: None,
            trash_mailbox_id: None,
            admin_user: admin_user.to_string(),
            admin_pass: admin_pass.to_string(),
        };

        // Discover special mailboxes
        client.drafts_mailbox_id = client.find_mailbox_by_role("drafts").await.ok().flatten();
        client.trash_mailbox_id = client.find_mailbox_by_role("trash").await.ok().flatten();
        debug!(drafts = ?client.drafts_mailbox_id, trash = ?client.trash_mailbox_id, "mailbox discovery complete");

        Ok(client)
    }

    async fn find_mailbox_by_role(&self, role: &str) -> Result<Option<String>> {
        let resp = self
            .call(vec![(
                "Mailbox/get".into(),
                json!({
                    "accountId": self.account_id,
                    "properties": ["id", "name", "role"],
                }),
                "m0".into(),
            )])
            .await?;

        for (method, result, _) in &resp.method_responses {
            if method == "Mailbox/get" {
                if let Some(list) = result.get("list").and_then(|l| l.as_array()) {
                    for mb in list {
                        if mb.get("role").and_then(|r| r.as_str()) == Some(role) {
                            return Ok(mb.get("id").and_then(|id| id.as_str()).map(String::from));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    async fn call(&self, method_calls: Vec<(String, Value, String)>) -> Result<JmapResponse> {
        let req = JmapRequest {
            using: vec![
                "urn:ietf:params:jmap:core",
                "urn:ietf:params:jmap:mail",
                "urn:ietf:params:jmap:submission",
            ],
            method_calls,
        };

        let resp = self
            .http
            .post(&self.endpoint)
            .basic_auth(&self.admin_user, Some(&self.admin_pass))
            .json(&req)
            .send()
            .await?
            .error_for_status()?;

        let jmap_resp: JmapResponse = resp.json().await?;
        Ok(jmap_resp)
    }

    #[instrument(skip(self))]
    pub async fn email_search(
        &self,
        query: Option<&str>,
        mailbox: Option<&str>,
        from: Option<&str>,
        after: Option<&str>,
        before: Option<&str>,
        limit: u32,
    ) -> Result<Vec<EmailSummary>> {
        let filter = build_filter(query, mailbox, from, after, before);

        let mut query_args = json!({
            "accountId": self.account_id,
            "sort": [{ "property": "receivedAt", "isAscending": false }],
            "limit": limit,
        });
        if !filter.is_null() {
            query_args["filter"] = filter;
        }

        // Use back-reference: query first, then get with result reference
        let resp = self
            .call(vec![
                (
                    "Email/query".into(),
                    query_args,
                    "q0".into(),
                ),
                (
                    "Email/get".into(),
                    json!({
                        "accountId": self.account_id,
                        "#ids": {
                            "resultOf": "q0",
                            "name": "Email/query",
                            "path": "/ids"
                        },
                        "properties": ["id", "from", "to", "subject", "receivedAt", "preview", "keywords"],
                    }),
                    "g0".into(),
                ),
            ])
            .await?;

        // Find the Email/get response
        for (method, result, _) in &resp.method_responses {
            if method == "Email/get" {
                let list: Vec<Value> = result
                    .get("list")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();

                return Ok(list.iter().filter_map(parse_email_summary).collect());
            }
        }

        Ok(vec![])
    }

    #[instrument(skip(self))]
    pub async fn email_read(&self, id: &str, html: bool) -> Result<Option<EmailDetail>> {
        let mut args = json!({
            "accountId": self.account_id,
            "ids": [id],
            "properties": [
                "id", "from", "to", "cc", "subject", "receivedAt",
                "textBody", "htmlBody", "attachments", "bodyValues",
                "messageId", "inReplyTo", "keywords"
            ],
        });

        if html {
            args["fetchHTMLBodyValues"] = json!(true);
        } else {
            args["fetchTextBodyValues"] = json!(true);
        }

        let resp = self
            .call(vec![("Email/get".into(), args, "g0".into())])
            .await?;

        for (method, result, _) in &resp.method_responses {
            if method == "Email/get" {
                let list = result
                    .get("list")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();

                return Ok(list.first().map(|e| parse_email_detail(e, html)));
            }
        }

        Ok(None)
    }

    #[instrument(skip(self, body))]
    pub async fn email_send(
        &self,
        to: &[String],
        subject: &str,
        body: &str,
        cc: &[String],
        bcc: &[String],
        in_reply_to: Option<&str>,
        references: Option<&str>,
        format: &SendFormat,
    ) -> Result<String> {
        // Build mailboxIds — use drafts if discovered, otherwise empty (let server default)
        let mailbox_ids = if let Some(ref drafts_id) = self.drafts_mailbox_id {
            json!({ drafts_id.clone(): true })
        } else {
            json!({})
        };

        let is_html = matches!(format, SendFormat::Html);

        let mut email = json!({
            "from": [{"email": self.admin_user}],
            "to": to.iter().map(|a| json!({"email": a})).collect::<Vec<_>>(),
            "subject": subject,
            "mailboxIds": mailbox_ids,
            "bodyValues": {
                "body": { "value": body, "charset": "utf-8" }
            },
        });

        if is_html {
            email["htmlBody"] = json!([{ "partId": "body", "type": "text/html" }]);
        } else {
            email["textBody"] = json!([{ "partId": "body", "type": "text/plain" }]);
        }

        if !cc.is_empty() {
            email["cc"] = json!(cc.iter().map(|a| json!({"email": a})).collect::<Vec<_>>());
        }
        if !bcc.is_empty() {
            email["bcc"] = json!(bcc.iter().map(|a| json!({"email": a})).collect::<Vec<_>>());
        }
        if let Some(irt) = in_reply_to {
            email["inReplyTo"] = json!([irt]);
        }
        if let Some(refs) = references {
            email["references"] = json!(refs.split_whitespace().collect::<Vec<_>>());
        }

        let create_id = "draft1";

        let resp = self
            .call(vec![
                (
                    "Email/set".into(),
                    json!({
                        "accountId": self.account_id,
                        "create": { create_id: email },
                    }),
                    "c0".into(),
                ),
                (
                    "EmailSubmission/set".into(),
                    json!({
                        "accountId": self.account_id,
                        "create": {
                            "sub1": {
                                "emailId": format!("#{create_id}"),
                            },
                        },
                    }),
                    "s0".into(),
                ),
            ])
            .await?;

        for (method, result, _) in &resp.method_responses {
            if method == "Email/set" {
                if let Some(created) = result.get("created").and_then(|c| c.get(create_id)) {
                    if let Some(id) = created.get("id").and_then(|v| v.as_str()) {
                        return Ok(id.to_string());
                    }
                }
                if let Some(not_created) = result.get("notCreated").and_then(|c| c.get(create_id)) {
                    return Err(JmapError::Protocol(format!(
                        "failed to create email: {not_created}"
                    )));
                }
            }
        }

        Err(JmapError::UnexpectedResponse(
            "no email ID in Email/set response".into(),
        ))
    }

    #[instrument(skip(self))]
    pub async fn mailbox_list(&self) -> Result<Vec<Mailbox>> {
        let resp = self
            .call(vec![(
                "Mailbox/get".into(),
                json!({
                    "accountId": self.account_id,
                    "properties": ["id", "name", "role", "totalEmails", "unreadEmails"],
                }),
                "m0".into(),
            )])
            .await?;

        for (method, result, _) in &resp.method_responses {
            if method == "Mailbox/get" {
                if let Some(list) = result.get("list").and_then(|l| l.as_array()) {
                    return Ok(list.iter().filter_map(parse_mailbox).collect());
                }
            }
        }

        Ok(vec![])
    }

    #[instrument(skip(self))]
    pub async fn email_move(&self, id: &str, mailbox_id: &str) -> Result<()> {
        let resp = self
            .call(vec![(
                "Email/set".into(),
                json!({
                    "accountId": self.account_id,
                    "update": {
                        id: {
                            "mailboxIds": { mailbox_id: true }
                        }
                    }
                }),
                "u0".into(),
            )])
            .await?;

        for (method, result, _) in &resp.method_responses {
            if method == "Email/set" {
                if let Some(not_updated) = result.get("notUpdated").and_then(|n| n.get(id)) {
                    return Err(JmapError::Protocol(format!(
                        "failed to move email: {not_updated}"
                    )));
                }
            }
        }

        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn email_delete(&self, id: &str) -> Result<String> {
        // Check if email is already in trash — if so, destroy permanently
        // Otherwise, move to trash
        if let Some(ref trash_id) = self.trash_mailbox_id {
            // First check if the email is in trash
            let check_resp = self
                .call(vec![(
                    "Email/get".into(),
                    json!({
                        "accountId": self.account_id,
                        "ids": [id],
                        "properties": ["mailboxIds"],
                    }),
                    "c0".into(),
                )])
                .await?;

            let mut in_trash = false;
            for (method, result, _) in &check_resp.method_responses {
                if method == "Email/get" {
                    if let Some(list) = result.get("list").and_then(|l| l.as_array()) {
                        if let Some(email) = list.first() {
                            if let Some(mbox_ids) = email.get("mailboxIds") {
                                in_trash = mbox_ids.get(trash_id).is_some();
                            }
                        }
                    }
                }
            }

            if in_trash {
                // Permanently destroy
                let resp = self
                    .call(vec![(
                        "Email/set".into(),
                        json!({
                            "accountId": self.account_id,
                            "destroy": [id],
                        }),
                        "d0".into(),
                    )])
                    .await?;

                for (method, result, _) in &resp.method_responses {
                    if method == "Email/set" {
                        if let Some(not_destroyed) =
                            result.get("notDestroyed").and_then(|n| n.get(id))
                        {
                            return Err(JmapError::Protocol(format!(
                                "failed to destroy email: {not_destroyed}"
                            )));
                        }
                    }
                }

                return Ok("destroyed".to_string());
            }

            // Move to trash
            self.email_move(id, trash_id).await?;
            return Ok("trashed".to_string());
        }

        // No trash mailbox found — destroy permanently
        let resp = self
            .call(vec![(
                "Email/set".into(),
                json!({
                    "accountId": self.account_id,
                    "destroy": [id],
                }),
                "d0".into(),
            )])
            .await?;

        for (method, result, _) in &resp.method_responses {
            if method == "Email/set" {
                if let Some(not_destroyed) = result.get("notDestroyed").and_then(|n| n.get(id)) {
                    return Err(JmapError::Protocol(format!(
                        "failed to destroy email: {not_destroyed}"
                    )));
                }
            }
        }

        Ok("destroyed".to_string())
    }

    #[instrument(skip(self))]
    pub async fn email_update_keywords(
        &self,
        id: &str,
        is_read: Option<bool>,
        is_flagged: Option<bool>,
    ) -> Result<()> {
        let mut update = json!({});
        if let Some(read) = is_read {
            update["keywords/$seen"] = json!(read);
        }
        if let Some(flagged) = is_flagged {
            update["keywords/$flagged"] = json!(flagged);
        }

        let resp = self
            .call(vec![(
                "Email/set".into(),
                json!({
                    "accountId": self.account_id,
                    "update": { id: update }
                }),
                "u0".into(),
            )])
            .await?;

        for (method, result, _) in &resp.method_responses {
            if method == "Email/set" {
                if let Some(not_updated) = result.get("notUpdated").and_then(|n| n.get(id)) {
                    return Err(JmapError::Protocol(format!(
                        "failed to update email: {not_updated}"
                    )));
                }
            }
        }

        Ok(())
    }
}

fn build_filter(
    query: Option<&str>,
    mailbox: Option<&str>,
    from: Option<&str>,
    after: Option<&str>,
    before: Option<&str>,
) -> Value {
    let mut conditions: Vec<Value> = Vec::new();

    if let Some(q) = query {
        conditions.push(json!({ "text": q }));
    }
    if let Some(m) = mailbox {
        conditions.push(json!({ "inMailbox": m }));
    }
    if let Some(f) = from {
        conditions.push(json!({ "from": f }));
    }
    if let Some(a) = after {
        conditions.push(json!({ "after": a }));
    }
    if let Some(b) = before {
        conditions.push(json!({ "before": b }));
    }

    match conditions.len() {
        0 => Value::Null,
        1 => conditions.into_iter().next().unwrap(),
        _ => json!({
            "operator": "AND",
            "conditions": conditions,
        }),
    }
}

fn parse_jmap_addresses(val: &Value) -> Vec<Address> {
    val.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    Some(Address {
                        name: a.get("name").and_then(|n| n.as_str()).map(String::from),
                        email: a.get("email")?.as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_keywords(val: &Value) -> (bool, bool) {
    let empty = json!({});
    let keywords = val.get("keywords").unwrap_or(&empty);
    let is_read = keywords.get("$seen").and_then(|v| v.as_bool()).unwrap_or(false);
    let is_flagged = keywords.get("$flagged").and_then(|v| v.as_bool()).unwrap_or(false);
    (is_read, is_flagged)
}

fn parse_email_summary(val: &Value) -> Option<EmailSummary> {
    let (is_read, is_flagged) = parse_keywords(val);
    Some(EmailSummary {
        id: val.get("id")?.as_str()?.to_string(),
        from: parse_jmap_addresses(val.get("from").unwrap_or(&json!([]))),
        to: parse_jmap_addresses(val.get("to").unwrap_or(&json!([]))),
        subject: val
            .get("subject")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string(),
        received_at: val
            .get("receivedAt")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string(),
        preview: val
            .get("preview")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string(),
        is_read,
        is_flagged,
    })
}

fn parse_email_detail(val: &Value, html: bool) -> EmailDetail {
    let empty = json!({});
    let body_values = val.get("bodyValues").unwrap_or(&empty);

    let body_key = if html { "htmlBody" } else { "textBody" };
    let body = extract_body_value(val, body_key, body_values);

    let attachments = val
        .get("attachments")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .map(|a| Attachment {
                    name: a
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("unnamed")
                        .to_string(),
                    size: a.get("size").and_then(|s| s.as_u64()).unwrap_or(0),
                    content_type: a
                        .get("type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("application/octet-stream")
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    let message_id = val
        .get("messageId")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .map(String::from);

    let in_reply_to = val
        .get("inReplyTo")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .map(String::from);

    EmailDetail {
        id: val
            .get("id")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string(),
        from: parse_jmap_addresses(val.get("from").unwrap_or(&json!([]))),
        to: parse_jmap_addresses(val.get("to").unwrap_or(&json!([]))),
        cc: parse_jmap_addresses(val.get("cc").unwrap_or(&json!([]))),
        subject: val
            .get("subject")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string(),
        received_at: val
            .get("receivedAt")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string(),
        body,
        attachments,
        message_id,
        in_reply_to,
    }
}

fn parse_mailbox(val: &Value) -> Option<Mailbox> {
    Some(Mailbox {
        id: val.get("id")?.as_str()?.to_string(),
        name: val
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string(),
        role: val.get("role").and_then(|r| r.as_str()).map(String::from),
        total_emails: val
            .get("totalEmails")
            .and_then(|n| n.as_u64())
            .unwrap_or(0),
        unread_emails: val
            .get("unreadEmails")
            .and_then(|n| n.as_u64())
            .unwrap_or(0),
    })
}

fn extract_body_value(email: &Value, body_key: &str, body_values: &Value) -> String {
    email
        .get(body_key)
        .and_then(|parts| parts.as_array())
        .and_then(|parts| parts.first())
        .and_then(|part| part.get("partId"))
        .and_then(|id| id.as_str())
        .and_then(|part_id| body_values.get(part_id))
        .and_then(|bv| bv.get("value"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}
