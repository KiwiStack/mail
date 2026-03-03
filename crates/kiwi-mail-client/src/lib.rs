use serde::de::DeserializeOwned;

use kiwi_mail_types::{
    EmailDetail, EmailSummary, KiwiErrorResponse, KiwiResponse, MailSearchRequest, MailSendRequest,
    MailSendResponse,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API error ({code}): {message}")]
    Api { code: String, message: String },
}

type Result<T> = std::result::Result<T, Error>;

pub struct KiwiMailClient {
    http: reqwest::Client,
    base_url: String,
}

impl KiwiMailClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    async fn handle_response<T: DeserializeOwned>(&self, resp: reqwest::Response) -> Result<T> {
        if resp.status().is_success() {
            let body: KiwiResponse<T> = resp.json().await?;
            Ok(body.data)
        } else {
            let err: KiwiErrorResponse = resp.json().await.map_err(Error::Http)?;
            Err(Error::Api {
                code: err.error.code,
                message: err.error.message,
            })
        }
    }

    pub async fn search(&self, req: &MailSearchRequest) -> Result<Vec<EmailSummary>> {
        let resp = self
            .http
            .post(format!("{}/api/v1/mail/search", self.base_url))
            .json(req)
            .send()
            .await?;

        self.handle_response(resp).await
    }

    pub async fn read(&self, id: &str) -> Result<EmailDetail> {
        let resp = self
            .http
            .get(format!("{}/api/v1/mail/{}", self.base_url, id))
            .send()
            .await?;

        self.handle_response(resp).await
    }

    pub async fn send(&self, req: &MailSendRequest) -> Result<MailSendResponse> {
        let resp = self
            .http
            .post(format!("{}/api/v1/mail/send", self.base_url))
            .json(req)
            .send()
            .await?;

        self.handle_response(resp).await
    }
}
