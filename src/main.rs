use std::{
    error::Error,
    io::{self, BufRead},
};

use dotenv::dotenv;
use reqwest::Client;
use serde::Deserialize;

extern crate imap;
extern crate native_tls;
extern crate rpassword;

struct OAuth2 {
    user: String,
    access_token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OAuthResponse {
    pub access_token: String,
}

pub async fn request_google_oauth_token(
    authorization_code: &str,
) -> Result<OAuthResponse, Box<dyn Error>> {
    dotenv().ok().unwrap();
    let client_id = dotenv::var("CLIENT_ID").unwrap();
    let client_secret = dotenv::var("CLIENT_SECRET").unwrap();
    let redirect_url = "urn:ietf:wg:oauth:2.0:oob";
    let root_url = "https://oauth2.googleapis.com/token";
    let client = Client::new();

    let params = [
        ("grant_type", "authorization_code"),
        ("redirect_uri", redirect_url),
        ("client_id", client_id.as_str()),
        ("client_secret", client_secret.as_str()),
        ("code", authorization_code),
    ];

    let response = client.post(root_url).form(&params).send().await?;

    if response.status().is_success() {
        let oauth_response = response.json::<OAuthResponse>().await?;
        Ok(oauth_response)
    } else {
        let message = "An error occurred while trying to retrieve access token.";
        Err(From::from(message))
    }
}

impl imap::Authenticator for OAuth2 {
    type Response = String;
    fn process(&self, _: &[u8]) -> Self::Response {
        format!(
            "user={}\x01auth=Bearer {}\x01\x01",
            self.user, self.access_token
        )
    }
}

fn fetch_inbox_top(access_token: String) -> imap::error::Result<Option<String>> {
    let domain = "imap.gmail.com";
    let tls = native_tls::TlsConnector::builder().build().unwrap();

    // we pass in the domain twice to check that the server's TLS
    // certificate is valid for the domain we're connecting to.
    let client = imap::connect((domain, 993), domain, &tls).unwrap();

    let auth = OAuth2 {
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

fn get_token_request_url() -> String {
    let client_id = dotenv::var("CLIENT_ID").unwrap();
    let redirect_url = "urn:ietf:wg:oauth:2.0:oob";
    let scopes = "https://mail.google.com";

    format!(
        "https://accounts.google.com/o/oauth2/v2/auth\
          ?access_type=offline\
          &client_id={client_id}\
          &redirect_uri={redirect_url}\
          &response_type=code\
          &scope={scopes}"
    )
}

#[tokio::main]
async fn main() {
    println!("{}", get_token_request_url());
    println!("paste code here:");
    let stdin = io::stdin();
    let code = stdin
        .lock()
        .lines()
        .next()
        .expect("there was no next line")
        .expect("the line could not be read");

    let res = request_google_oauth_token(&code).await.unwrap();
    let msg = fetch_inbox_top(res.access_token).unwrap().unwrap();
    println!("{msg}");
}
