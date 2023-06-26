use std::{
    fs,
    io::{stdin, BufRead},
    net::TcpStream,
};

use anyhow::anyhow;
use clap::{Parser, Subcommand};
use imap::Session;
use native_tls::TlsStream;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::google::{
    refresh_google_oauth_token, request_google_oauth_token, GoogleOAuthParams,
    GoogleOAuthTokenRefreshResponse, GoogleOAuthTokenRequestResponse, GOOGLE_IMAP_DOMAIN,
    GOOGLE_IMAP_PORT,
};

extern crate imap;
extern crate native_tls;
extern crate rpassword;

mod google;

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(about = "login to email", long_about = "login to email")]
    Login { email: String },
    #[command(about = "read emails", long_about = "read emails")]
    Read { n: u32 },
}

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Serialize, Deserialize)]
struct UserData {
    email: String,
    access_token: String,
    refresh_token: String,
}

struct ImapOAuth2 {
    user: String,
    access_token: String,
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

/// Errors: if credentials are invalid or access token is expired
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
    let args = CliArgs::parse();

    match args.command {
        Commands::Login { email } => {
            let auth_params = GoogleOAuthParams::default();

            println!("{}", auth_params.get_token_request_url());
            println!("paste code here:");

            let stdin = stdin();
            let code = stdin
                .lock()
                .lines()
                .next()
                .expect("there was no next line")?;

            let client = Client::new();

            let GoogleOAuthTokenRequestResponse {
                access_token,
                refresh_token,
            } = request_google_oauth_token(&client, &auth_params, &code).await?;

            if let Some(base_dir) = directories::BaseDirs::new() {
                let data_dir = base_dir.data_dir().join("mail-cli/");
                let data = UserData {
                    email,
                    access_token,
                    refresh_token,
                };

                fs::create_dir_all(&data_dir)?;
                fs::write(&data_dir.join("user.toml"), toml::to_string_pretty(&data)?)?;
            } else {
                todo!();
            }
        }
        Commands::Read { n } => {
            if let Some(base_dir) = directories::BaseDirs::new() {
                let data_file = base_dir.data_dir().join("mail-cli/user.toml");
                let data_str = match fs::read_to_string(data_file) {
                    Ok(content) => content,
                    Err(err) => match err.kind() {
                        std::io::ErrorKind::NotFound => return Err(anyhow!( "you need to login before you can use this command: run `mail-cli login <email>`")),
                        _ => return Err(err.into())
                    },
                };

                let UserData {
                    email,
                    access_token,
                    refresh_token,
                } = toml::from_str(&data_str)?;

                let imap_auth = ImapOAuth2 {
                    user: email.clone(),
                    access_token,
                };

                let mut session =
                    match create_imap_session(GOOGLE_IMAP_DOMAIN, GOOGLE_IMAP_PORT, &imap_auth) {
                        Ok(session) => session,
                        Err(_) => {
                            let GoogleOAuthTokenRefreshResponse { access_token } =
                                refresh_google_oauth_token(
                                    &Client::new(),
                                    &GoogleOAuthParams::default(),
                                    &refresh_token,
                                )
                                .await?;

                            // TODO update user.toml file

                            let imap_auth = ImapOAuth2 {
                                user: email,
                                access_token,
                            };

                            create_imap_session(GOOGLE_IMAP_DOMAIN, GOOGLE_IMAP_PORT, &imap_auth)?
                        }
                    };

                let msg = fetch_top_n_msg_from_inbox(&mut session, n)?;

                println!("{msg:?}");

                session.logout()?;
            } else {
                todo!();
            }
        }
    }

    Ok(())
}
