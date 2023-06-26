use std::io::{self, BufRead};

use anyhow::{anyhow, Ok};
use dotenv::dotenv;
use reqwest::{Client, StatusCode};
use serde::Deserialize;

extern crate imap;
extern crate native_tls;
extern crate rpassword;

static GOOGLE_AUTH_ROOT_URL: &str = "https://oauth2.googleapis.com/token";

struct ImapOAuth2 {
    user: String,
    access_token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GoogleOAuthResponse {
    pub access_token: String,
}

#[derive(Debug, Clone)]
pub struct GoogleOAuthParams {
    client_id: String,
    client_secret: String,
    redirect_url: String,
    grant_type: String,
    scopes: String,
}

impl imap::Authenticator for ImapOAuth2 {
    type Response = String;
    fn process(&self, _: &[u8]) -> Self::Response {
        format!(
            "user={}\x01auth=Bearer {}\x01\x01",
            self.user, self.access_token
        )
    }
}

impl GoogleOAuthParams {
    fn new(client_id: String, client_secret: String) -> Self {
        Self {
            client_id,
            client_secret,
            redirect_url: "urn:ietf:wg:oauth:2.0:oob".to_owned(),
            grant_type: "authorization_code".to_owned(),
            scopes: "https://mail.google.com".to_owned(),
        }
    }

    fn to_form_params<'a>(&'a self, auth_code: &'a str) -> [(&'a str, &'a str); 5] {
        [
            ("grant_type", &self.grant_type),
            ("redirect_uri", &self.redirect_url),
            ("client_id", &self.client_id),
            ("client_secret", &self.client_secret),
            ("code", auth_code),
        ]
    }

    fn get_token_request_url(&self) -> String {
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
    dotenv().ok().unwrap();
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

fn fetch_inbox_top(access_token: String) -> anyhow::Result<Option<String>> {
    let domain = "imap.gmail.com";
    let tls = native_tls::TlsConnector::builder().build().unwrap();

    // we pass in the domain twice to check that the server's TLS
    // certificate is valid for the domain we're connecting to.
    let client = imap::connect((domain, 993), domain, &tls).unwrap();

    let auth = ImapOAuth2 {
        user: "gschwantnermoritz@gmail.com".to_owned(),
        access_token: access_token.to_owned(),
    };

    let mut session = client.authenticate("XOAUTH2", &auth).unwrap();
    // we want to fetch the first email in the INBOX mailbox
    session.select("INBOX")?;

    // fetch message number 1 in this mailbox, along with its RFC822 field.
    // RFC 822 dictates the format of the body of e-mails
    let messages = session.fetch("1", "RFC822")?;
    let message = if let Some(m) = messages.iter().next() {
        m
    } else {
        return Ok(None);
    };

    // extract the message's body
    let body = message.body().expect("message did not have a body!");
    let body = std::str::from_utf8(body)
        .expect("message was not valid utf-8")
        .to_string();

    // be nice to the server and log out
    session.logout()?;

    Ok(Some(body))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let auth_params =
        GoogleOAuthParams::new(dotenv::var("CLIENT_ID")?, dotenv::var("CLIENT_SECRET")?);

    println!("{}", auth_params.get_token_request_url());
    println!("paste code here:");

    let stdin = io::stdin();
    let code = stdin
        .lock()
        .lines()
        .next()
        .expect("there was no next line")?;

    let res = request_google_oauth_token(&auth_params, &code).await?;
    let msg = fetch_inbox_top(res.access_token).unwrap().unwrap();

    println!("{msg}");

    Ok(())
}
