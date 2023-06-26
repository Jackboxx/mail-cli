use anyhow::anyhow;
use reqwest::{Client, StatusCode};
use serde::Deserialize;

pub static GOOGLE_CLIENT_ID: &str =
    "595889029500-45v36gai2da7jh6io8h7f2077bfv8cd2.apps.googleusercontent.com";
pub static GOOGLE_CLIENT_SECRET: &str = "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";
pub static GOOGLE_AUTH_ROOT_URL: &str = "https://oauth2.googleapis.com/token";
pub static GOOGLE_IMAP_DOMAIN: &str = "imap.gmail.com";
pub static GOOGLE_IMAP_PORT: u16 = 993;

#[derive(Debug, Clone, Deserialize)]
pub struct GoogleOAuthResponse {
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Debug, Clone)]
pub struct GoogleOAuthParams {
    client_id: String,
    client_secret: String,
    redirect_url: String,
    grant_type: String,
    scopes: String,
}

impl Default for GoogleOAuthParams {
    fn default() -> Self {
        Self {
            client_id: GOOGLE_CLIENT_ID.to_owned(),
            client_secret: GOOGLE_CLIENT_SECRET.to_owned(),
            redirect_url: "urn:ietf:wg:oauth:2.0:oob".to_owned(),
            grant_type: "authorization_code".to_owned(),
            scopes: "https://mail.google.com".to_owned(),
        }
    }
}

impl GoogleOAuthParams {
    pub fn to_form_params<'a>(&'a self, auth_code: &'a str) -> [(&'a str, &'a str); 5] {
        [
            ("grant_type", &self.grant_type),
            ("redirect_uri", &self.redirect_url),
            ("client_id", &self.client_id),
            ("client_secret", &self.client_secret),
            ("code", auth_code),
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
    auth_params: &GoogleOAuthParams,
    auth_code: &str,
) -> anyhow::Result<GoogleOAuthResponse> {
    let client = Client::new();

    let res = client
        .post(GOOGLE_AUTH_ROOT_URL)
        .form(&auth_params.to_form_params(auth_code))
        .send()
        .await?;

    match res.status() {
        StatusCode::OK => Ok(res.json::<GoogleOAuthResponse>().await?),
        _ => Err(anyhow!(
            "an error occurred while trying to retrieve access token",
        )),
    }
}
