// downloaders/pikpak/src/pikpak_api.rs
//! Raw PikPak HTTP API client.
//! Reference: https://github.com/Bengerthelorf/pikpaktui/blob/main/src/pikpak.rs

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

const AUTH_BASE_URL: &str = "https://user.mypikpak.com";
const DRIVE_BASE_URL: &str = "https://api-drive.mypikpak.com";
const CLIENT_ID: &str = "YNxT9w7GMdWvEOKa";
const CLIENT_SECRET: &str = "dbw2OtmVEeuUvIptb1Coyg";
const USER_AGENT: &str = "ANDROID-com.pikcloud.pikpak/1.21.0";

#[derive(Debug, Clone)]
struct Token {
    access_token: String,
    refresh_token: String,
    expires_at: u64,
}

impl Token {
    fn is_expiring(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now + 300 >= self.expires_at
    }
}

#[derive(Debug, Deserialize)]
struct SignInResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
}

#[derive(Debug, Deserialize)]
struct CaptchaInitResponse {
    captcha_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineTask {
    pub id: String,
    pub name: Option<String>,
    pub phase: String,
    pub progress: Option<i64>,
    pub file_id: Option<String>,
    pub file_size: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OfflineTaskResponse {
    pub task: Option<OfflineTask>,
}

#[derive(Debug, Deserialize)]
struct OfflineListResponse {
    pub tasks: Option<Vec<OfflineTask>>,
}

#[derive(Debug, Deserialize)]
pub struct FileInfo {
    pub id: String,
    pub name: Option<String>,
    pub size: Option<String>,
    pub web_content_link: Option<String>,
    pub links: Option<FileLinks>,
}

#[derive(Debug, Deserialize)]
pub struct FileLinks {
    #[serde(rename = "application/octet-stream")]
    pub download: Option<FileLink>,
}

#[derive(Debug, Deserialize)]
pub struct FileLink {
    pub url: String,
}

#[derive(Clone)]
pub struct PikPakApi {
    http: Client,
    token: Arc<RwLock<Option<Token>>>,
}

impl PikPakApi {
    pub fn new() -> Self {
        let http = Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .expect("Failed to build reqwest client");
        Self {
            http,
            token: Arc::new(RwLock::new(None)),
        }
    }

    fn make_device_id(email: &str) -> String {
        use sha2::{Digest, Sha256};
        let result = Sha256::digest(email.as_bytes());
        hex::encode(&result[..16])
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<()> {
        let device_id = Self::make_device_id(email);
        let captcha_resp: CaptchaInitResponse = self
            .http
            .post(format!("{AUTH_BASE_URL}/v1/shield/captcha/init"))
            .json(&serde_json::json!({
                "client_id": CLIENT_ID,
                "action": "POST:/v1/auth/signin",
                "device_id": device_id,
                "meta": { "email": email }
            }))
            .send()
            .await?
            .json()
            .await?;

        let signin_resp: SignInResponse = self
            .http
            .post(format!("{AUTH_BASE_URL}/v1/auth/signin"))
            .header("x-device-id", &device_id)
            .header("x-captcha-token", &captcha_resp.captcha_token)
            .json(&serde_json::json!({
                "client_id": CLIENT_ID,
                "client_secret": CLIENT_SECRET,
                "username": email,
                "password": password
            }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| {
                tracing::warn!("SystemTime before UNIX_EPOCH; using zero as fallback");
                std::time::Duration::ZERO
            })
            .as_secs()
            + signin_resp.expires_in;

        *self.token.write().await = Some(Token {
            access_token: signin_resp.access_token,
            refresh_token: signin_resp.refresh_token,
            expires_at,
        });

        tracing::info!("PikPak login successful for {email}");
        Ok(())
    }

    async fn refresh_if_needed(&self) -> Result<String> {
        // Use write lock directly to prevent concurrent refresh race condition.
        let mut token_guard = self.token.write().await;

        if let Some(token) = token_guard.as_ref() {
            if !token.is_expiring() {
                return Ok(token.access_token.clone());
            }
        }

        let refresh_token = token_guard
            .as_ref()
            .ok_or_else(|| anyhow!("Not logged in"))?
            .refresh_token
            .clone();

        let resp: SignInResponse = self
            .http
            .post(format!("{AUTH_BASE_URL}/v1/auth/token"))
            .json(&serde_json::json!({
                "client_id": CLIENT_ID,
                "client_secret": CLIENT_SECRET,
                "grant_type": "refresh_token",
                "refresh_token": refresh_token
            }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| {
                tracing::warn!("SystemTime before UNIX_EPOCH; using zero as fallback");
                std::time::Duration::ZERO
            })
            .as_secs();
        let expires_at = now + resp.expires_in;

        let new_access = resp.access_token.clone();
        *token_guard = Some(Token {
            access_token: resp.access_token,
            refresh_token: resp.refresh_token,
            expires_at,
        });
        Ok(new_access)
    }

    pub async fn offline_download(&self, url: &str) -> Result<OfflineTask> {
        let access_token = self.refresh_if_needed().await?;
        let resp: OfflineTaskResponse = self
            .http
            .post(format!("{DRIVE_BASE_URL}/drive/v1/files"))
            .bearer_auth(&access_token)
            .json(&serde_json::json!({
                "kind": "drive#file",
                "upload_type": "UPLOAD_TYPE_URL",
                "url": { "url": url },
                "folder_type": "DOWNLOAD"
            }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        resp.task.ok_or_else(|| anyhow!("PikPak returned no task for offline download"))
    }

    pub async fn list_running_tasks(&self) -> Result<Vec<OfflineTask>> {
        self.list_tasks_by_phase("PHASE_TYPE_RUNNING").await
    }

    pub async fn list_completed_tasks(&self) -> Result<Vec<OfflineTask>> {
        self.list_tasks_by_phase("PHASE_TYPE_COMPLETE").await
    }

    async fn list_tasks_by_phase(&self, phase: &str) -> Result<Vec<OfflineTask>> {
        let access_token = self.refresh_if_needed().await?;
        let filters = serde_json::json!({ "phase": { "in": phase } });
        let resp: OfflineListResponse = self
            .http
            .get(format!("{DRIVE_BASE_URL}/drive/v1/tasks"))
            .bearer_auth(&access_token)
            .query(&[
                ("type", "offline"),
                ("thumbnail_size", "SIZE_SMALL"),
                ("limit", "200"),
                ("filters", &filters.to_string()),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(resp.tasks.unwrap_or_default())
    }

    pub async fn get_file_download_url(&self, file_id: &str) -> Result<(String, u64)> {
        let access_token = self.refresh_if_needed().await?;
        let info: FileInfo = self
            .http
            .get(format!("{DRIVE_BASE_URL}/drive/v1/files/{file_id}"))
            .bearer_auth(&access_token)
            .query(&[("thumbnail_size", "SIZE_SMALL")])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let url = info
            .links
            .and_then(|l| l.download)
            .map(|l| l.url)
            .or(info.web_content_link)
            .ok_or_else(|| anyhow!("No download URL for file {file_id}"))?;

        let size = info
            .size
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        Ok((url, size))
    }

    pub async fn delete_tasks(&self, task_ids: &[&str], delete_files: bool) -> Result<()> {
        if task_ids.is_empty() {
            return Ok(());
        }
        let access_token = self.refresh_if_needed().await?;
        let ids = task_ids.join(",");
        self.http
            .delete(format!("{DRIVE_BASE_URL}/drive/v1/tasks"))
            .bearer_auth(&access_token)
            .query(&[
                ("task_ids", ids.as_str()),
                ("delete_files", if delete_files { "true" } else { "false" }),
            ])
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub fn is_logged_in(&self) -> bool {
        self.token.try_read().map(|g| g.is_some()).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_not_expiring() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let fresh = Token {
            access_token: "tok".into(),
            refresh_token: "ref".into(),
            expires_at: now + 3600,
        };
        assert!(!fresh.is_expiring());
    }

    #[test]
    fn test_token_expiring_soon() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let expiring = Token {
            access_token: "tok".into(),
            refresh_token: "ref".into(),
            expires_at: now + 100,
        };
        assert!(expiring.is_expiring());
    }

    #[test]
    fn test_device_id_deterministic() {
        let a = PikPakApi::make_device_id("test@example.com");
        let b = PikPakApi::make_device_id("test@example.com");
        assert_eq!(a, b);
        let c = PikPakApi::make_device_id("other@example.com");
        assert_ne!(a, c);
    }
}
