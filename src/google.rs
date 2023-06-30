use anyhow::anyhow;
use reqwest::{Client, StatusCode};
use serde::Deserialize;

pub static GOOGLE_AUTH_ROOT_URL: &str = "https://oauth2.googleapis.com/token";
pub static GOOGLE_IMAP_DOMAIN: &str = "imap.gmail.com";
pub static GOOGLE_IMAP_PORT: u16 = 993;

#[derive(Debug, Clone, Deserialize)]
pub struct GoogleOAuthTokenRequestResponse {
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GoogleOAuthTokenRefreshResponse {
    pub access_token: String,
}

#[derive(Debug, Clone)]
pub struct GoogleOAuthParams {
    client_id: String,
    client_secret: String,
    redirect_url: String,
    scopes: String,
}

impl Default for GoogleOAuthParams {
    /// loads `client_id` and `client_secret` from `.env` file
    ///
    /// Panics:
    /// - if it can't load the `GOOGLE_CLIENT_ID` or `GOOGLE_CLIENT_SECRET` environment variables
    fn default() -> Self {
        let client_id = dotenv::var("GOOGLE_CLIENT_ID").expect("failed to load GOOGLE_CLIENT_ID");
        let client_secret =
            dotenv::var("GOOGLE_CLIENT_SECRET").expect("failed to load GOOGLE_CLIENT_SECRET");

        Self {
            client_id,
            client_secret,
            redirect_url: "urn:ietf:wg:oauth:2.0:oob".to_owned(),
            scopes: "https://mail.google.com".to_owned(),
        }
    }
}

impl GoogleOAuthParams {
    pub fn to_form_request_params<'a>(&'a self, auth_code: &'a str) -> [(&'a str, &'a str); 5] {
        [
            ("grant_type", "authorization_code"),
            ("redirect_uri", &self.redirect_url),
            ("client_id", &self.client_id),
            ("client_secret", &self.client_secret),
            ("code", auth_code),
        ]
    }

    pub fn to_form_refresh_params<'a>(&'a self, refresh_token: &'a str) -> [(&'a str, &'a str); 4] {
        [
            ("grant_type", "refresh_token"),
            ("client_id", &self.client_id),
            ("client_secret", &self.client_secret),
            ("refresh_token", refresh_token),
        ]
    }

    pub fn get_token_request_url(&self) -> String {
        format!(
            "https://accounts.google.com/o/oauth2/v2/auth\
          ?access_type=offline\
          &client_id={id}\
          &redirect_uri={uri}\
          &response_type=code\
          &scope={scopes}",
            id = self.client_id,
            uri = self.redirect_url,
            scopes = self.scopes
        )
    }
}

pub async fn request_google_oauth_token(
    client: &Client,
    auth_params: &GoogleOAuthParams,
    auth_code: &str,
) -> anyhow::Result<GoogleOAuthTokenRequestResponse> {
    let res = client
        .post(GOOGLE_AUTH_ROOT_URL)
        .form(&auth_params.to_form_request_params(auth_code))
        .send()
        .await?;

    match res.status() {
        StatusCode::OK => Ok(res.json().await?),
        _ => Err(anyhow!(
            "an error occurred while trying to retrieve access token",
        )),
    }
}

pub async fn refresh_google_oauth_token(
    client: &Client,
    auth_params: &GoogleOAuthParams,
    refresh_token: &str,
) -> anyhow::Result<GoogleOAuthTokenRefreshResponse> {
    let res = client
        .post(GOOGLE_AUTH_ROOT_URL)
        .form(&auth_params.to_form_refresh_params(refresh_token))
        .send()
        .await?;

    match res.status() {
        StatusCode::OK => Ok(res.json().await?),
        _ => Err(anyhow!(
            "an error occurred while trying to retrieve access token, status code {status}",
            status = res.status().as_u16(),
        )),
    }
}
