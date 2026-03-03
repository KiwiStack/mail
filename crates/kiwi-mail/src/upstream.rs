use std::time::Duration;

use tokio::process::{Child, Command};
use tracing::{error, info, warn};

use crate::config::Config;

pub struct Upstream {
    child: Child,
    upstream_addr: String,
    admin_user: String,
    admin_pass: String,
    http: reqwest::Client,
}

impl Upstream {
    pub async fn spawn(config: &Config) -> anyhow::Result<Self> {
        info!(
            bin = %config.upstream_bin.display(),
            config = %config.upstream_config.display(),
            "spawning upstream"
        );

        let child = Command::new(&config.upstream_bin)
            .arg("--config")
            .arg(&config.upstream_config)
            .kill_on_drop(true)
            .spawn()?;

        info!(pid = child.id(), "upstream process started");

        let upstream = Self {
            child,
            upstream_addr: config.upstream_addr.clone(),
            admin_user: config.admin_user.clone(),
            admin_pass: config.admin_pass.clone(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()?,
        };

        upstream.wait_healthy(config).await?;

        Ok(upstream)
    }

    async fn wait_healthy(&self, config: &Config) -> anyhow::Result<()> {
        let timeout = config.health_check_timeout.0;
        let url = format!("{}/.well-known/jmap", self.upstream_addr);

        info!(%url, ?timeout, "waiting for upstream to become healthy");

        let start = tokio::time::Instant::now();
        let mut delay = Duration::from_millis(100);
        let max_delay = Duration::from_secs(5);

        loop {
            match self
                .http
                .get(&url)
                .basic_auth(&self.admin_user, Some(&self.admin_pass))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    info!(elapsed = ?start.elapsed(), "upstream is healthy");
                    return Ok(());
                }
                Ok(resp) => {
                    warn!(status = %resp.status(), "upstream not ready yet");
                }
                Err(e) => {
                    warn!(error = %e, "upstream health check failed");
                }
            }

            if start.elapsed() > timeout {
                anyhow::bail!("upstream failed to become healthy within {timeout:?}");
            }

            tokio::time::sleep(delay).await;
            delay = (delay * 2).min(max_delay);
        }
    }

    pub async fn check_health(&self) -> bool {
        let url = format!("{}/.well-known/jmap", self.upstream_addr);
        matches!(
            self.http
                .get(&url)
                .basic_auth(&self.admin_user, Some(&self.admin_pass))
                .send()
                .await,
            Ok(resp) if resp.status().is_success()
        )
    }

    pub async fn shutdown(mut self) {
        info!("shutting down upstream");

        if let Some(pid) = self.child.id() {
            unsafe {
                libc::kill(pid as i32, libc::SIGTERM);
            }

            match tokio::time::timeout(Duration::from_secs(10), self.child.wait()).await {
                Ok(Ok(status)) => {
                    info!(%status, "upstream exited gracefully");
                }
                Ok(Err(e)) => {
                    error!(error = %e, "error waiting for upstream");
                }
                Err(_) => {
                    warn!("upstream did not exit in time, sending SIGKILL");
                    if let Err(e) = self.child.kill().await {
                        error!(error = %e, "failed to kill upstream");
                    }
                }
            }
        }
    }
}
