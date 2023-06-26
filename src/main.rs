use std::{
    fs,
    io::{stdin, BufRead},
    net::TcpStream,
};

use anyhow::{anyhow, Ok};
use clap::{Parser, Subcommand};
use imap::Session;
use native_tls::TlsStream;
use serde::{Deserialize, Serialize};

use crate::google::{
    request_google_oauth_token, GoogleOAuthParams, GoogleOAuthResponse, GOOGLE_IMAP_DOMAIN,
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

            let GoogleOAuthResponse {
                access_token,
                refresh_token,
            } = request_google_oauth_token(&auth_params, &code).await?;

            if let Some(base_dir) = directories::BaseDirs::new() {
                let data_dir = base_dir.data_dir().join("mail-cli/");
                let data = UserData {
                    email,
                    access_token,
                    refresh_token,
                };

                fs::create_dir_all(&data_dir)?;
                fs::write(&data_dir.join("user.toml"), toml::to_string_pretty(&data)?)?;
            }

            // let imap_auth = ImapOAuth2 {
            //     user: email,
            //     access_token,
            // };

            // let mut session =
            //     create_imap_session(GOOGLE_IMAP_DOMAIN, GOOGLE_IMAP_PORT, &imap_auth)?;
            // let msg = fetch_top_n_msg_from_inbox(&mut session, 2)?;

            // println!("{msg:?}");

            // session.logout()?;
        }
    }

    Ok(())
}
