use serde::Deserialize;

use super::types::BaiduError;

/// Which upstream does biblio use to refresh the access token?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    /// Default: OpenList team operates `api.oplist.org/baiduyun/renewapi`
    /// with their own AppKey, so users only supply a refresh token. Zero
    /// setup beyond going through OpenList's authorize URL once.
    OpenListProxy,
    /// Advanced: user registered their own Baidu application at
    /// https://pan.baidu.com/union/console/ and supplies its client_id +
    /// client_secret. Independent of OpenList's infrastructure.
    SelfApp,
}

/// Credentials stored between refreshes. `refresh_token` rotates on every
/// call so the caller must persist the returned one or the next refresh
/// will fail.
#[derive(Debug, Clone)]
pub struct BaiduCredentials {
    pub auth_mode: AuthMode,
    pub refresh_token: String,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

/// Outcome of a token refresh. `refresh_token` is the *new* token to
/// persist; the old one may already be invalidated server-side.
#[derive(Debug, Clone)]
pub struct RefreshResult {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in_secs: i64,
}

const OPENLIST_PROXY_URL: &str = "https://api.oplist.org/baiduyun/renewapi";
const BAIDU_OAUTH_URL: &str = "https://openapi.baidu.com/oauth/2.0/token";

pub async fn refresh_access_token(
    creds: &BaiduCredentials,
) -> Result<RefreshResult, BaiduError> {
    match creds.auth_mode {
        AuthMode::OpenListProxy => refresh_via_openlist(&creds.refresh_token).await,
        AuthMode::SelfApp => {
            let client_id = creds
                .client_id
                .as_deref()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| BaiduError("client_id missing for SelfApp mode".into()))?;
            let client_secret = creds
                .client_secret
                .as_deref()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| BaiduError("client_secret missing for SelfApp mode".into()))?;
            refresh_via_baidu(&creds.refresh_token, client_id, client_secret).await
        }
    }
}

#[derive(Debug, Deserialize)]
struct BaiduOAuthResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    // Error path uses `error` + `error_description` instead.
    error: Option<String>,
    error_description: Option<String>,
}

async fn refresh_via_openlist(refresh_token: &str) -> Result<RefreshResult, BaiduError> {
    // OpenList's proxy accepts the same request shape as Baidu's own OAuth
    // endpoint — the `refresh_ui` (sic) query param is how it identifies
    // the user's refresh token. `driver_txt=baiduyun_go` picks the Go
    // refresh flow which returns OAuth-compatible JSON.
    let url = format!(
        "{OPENLIST_PROXY_URL}?refresh_ui={refresh_token}&server_use=true&driver_txt=baiduyun_go"
    );
    let resp = reqwest::Client::new()
        .post(&url)
        .send()
        .await?
        .error_for_status()?;
    let parsed: BaiduOAuthResponse = resp.json().await?;
    parsed.into_result("OpenList proxy refresh")
}

async fn refresh_via_baidu(
    refresh_token: &str,
    client_id: &str,
    client_secret: &str,
) -> Result<RefreshResult, BaiduError> {
    let url = format!(
        "{BAIDU_OAUTH_URL}?grant_type=refresh_token&refresh_token={refresh_token}\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let resp = reqwest::Client::new().get(&url).send().await?.error_for_status()?;
    let parsed: BaiduOAuthResponse = resp.json().await?;
    parsed.into_result("Baidu OAuth refresh")
}

impl BaiduOAuthResponse {
    fn into_result(self, label: &str) -> Result<RefreshResult, BaiduError> {
        if let Some(err) = self.error {
            let desc = self.error_description.unwrap_or_default();
            return Err(BaiduError(format!(
                "{label} failed: {err} — {desc}"
            )));
        }
        let access_token = self
            .access_token
            .ok_or_else(|| BaiduError(format!("{label}: missing access_token in response")))?;
        let refresh_token = self.refresh_token.ok_or_else(|| {
            BaiduError(format!("{label}: missing refresh_token in response"))
        })?;
        let expires_in = self.expires_in.unwrap_or(2_592_000); // Baidu default: 30 days
        Ok(RefreshResult {
            access_token,
            refresh_token,
            expires_in_secs: expires_in,
        })
    }
}
