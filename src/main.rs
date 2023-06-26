use std::{
    io::{self, BufRead},
    net::TcpStream,
};

use anyhow::{anyhow, Ok};
use dotenv::dotenv;
use imap::Session;
use native_tls::TlsStream;
use reqwest::{Client, StatusCode};
use serde::Deserialize;

extern crate imap;
extern crate native_tls;
extern crate rpassword;

static GOOGLE_AUTH_ROOT_URL: &str = "https://oauth2.googleapis.com/token";
static GOOGLE_IMAP_DOMAIN: &str = "imap.gmail.com";
static GOOGLE_IMAP_PORT: u16 = 993;

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

fn create_imap_session(
    domain: &str,
    port: u16,
    imap_auth: &ImapOAuth2,
) -> anyhow::Result<Session<TlsStream<TcpStream>>> {
    let tls = native_tls::TlsConnector::builder().build()?;
    let client = imap::connect((domain, port), domain, &tls)?;

    Ok(client
        .authenticate("XOAUTH2", imap_auth)
        .map_err(|err| anyhow!(format!("{err:?}")))?)
}

fn fetch_top_n_msg_from_inbox(
    session: &mut Session<TlsStream<TcpStream>>,
    n: u32,
) -> anyhow::Result<Vec<String>> {
    session.select("INBOX")?;

    let messages = session.fetch(format!("{n}"), "RFC822")?;
    let mails: Vec<_> = messages
        .into_iter()
        .map(|msg| match msg.body() {
            Some(body) => std::str::from_utf8(body).map_err(|err| anyhow!(err)),
            None => Err(anyhow!("no body for message: {msg:?}")),
        })
        .collect();

    for mail in mails.iter() {
        if let Err(err) = mail {
            return Err(anyhow!(format!("{err:?}")));
        }
    }

    let clean_mails = mails
        .into_iter()
        .flat_map(|mail| mail.map(|content| content.to_owned()))
        .collect();

    Ok(clean_mails)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    let stdin = io::stdin();

    println!("enter your email:");
    let email = stdin
        .lock()
        .lines()
        .next()
        .expect("there was no next line")?;

    let auth_params =
        GoogleOAuthParams::new(dotenv::var("CLIENT_ID")?, dotenv::var("CLIENT_SECRET")?);

    println!("{}", auth_params.get_token_request_url());
    println!("paste code here:");

    let code = stdin
        .lock()
        .lines()
        .next()
        .expect("there was no next line")?;

    let GoogleOAuthResponse { access_token } =
        request_google_oauth_token(&auth_params, &code).await?;

    let imap_auth = ImapOAuth2 {
        user: email,
        access_token,
    };

    let mut session = create_imap_session(GOOGLE_IMAP_DOMAIN, GOOGLE_IMAP_PORT, &imap_auth)?;
    let msg = fetch_top_n_msg_from_inbox(&mut session, 2)?;

    println!("{msg:?}");

    session.logout()?;
    Ok(())
}
