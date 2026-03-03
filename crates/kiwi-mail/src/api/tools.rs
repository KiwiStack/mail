use axum::Json;
use serde_json::json;

use kiwi_mail_types::ToolDefinition;

pub async fn list_tools() -> Json<Vec<ToolDefinition>> {
    Json(vec![
        ToolDefinition {
            name: "mail.search".to_string(),
            description: "Search emails by query, sender, date range, or mailbox".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Full-text search query"
                    },
                    "mailbox": {
                        "type": "string",
                        "description": "Mailbox ID to search within"
                    },
                    "from": {
                        "type": "string",
                        "description": "Filter by sender email address"
                    },
                    "after": {
                        "type": "string",
                        "description": "Only emails received after this ISO 8601 date"
                    },
                    "before": {
                        "type": "string",
                        "description": "Only emails received before this ISO 8601 date"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results (default: 20)",
                        "default": 20
                    }
                }
            }),
        },
        ToolDefinition {
            name: "mail.read".to_string(),
            description: "Read a specific email by ID, returning full body and attachments"
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "The email ID to read"
                    },
                    "format": {
                        "type": "string",
                        "enum": ["text", "html"],
                        "description": "Body format to return (default: text)",
                        "default": "text"
                    }
                },
                "required": ["id"]
            }),
        },
        ToolDefinition {
            name: "mail.send".to_string(),
            description: "Send an email to one or more recipients".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "to": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Recipient email addresses"
                    },
                    "subject": {
                        "type": "string",
                        "description": "Email subject line"
                    },
                    "body": {
                        "type": "string",
                        "description": "Email body (plain text)"
                    },
                    "cc": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "CC recipient email addresses"
                    },
                    "bcc": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "BCC recipient email addresses"
                    }
                },
                "required": ["to", "subject", "body"]
            }),
        },
    ])
}
