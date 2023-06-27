use std::{
    collections::HashMap,
    fs,
    io::{stdin, BufRead},
    net::TcpStream,
    path::PathBuf,
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
struct StoredAccountData {
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

async fn add_new_account(
    email: String,
    existing_accounts: &mut HashMap<String, StoredAccountData>,
) -> anyhow::Result<()> {
    if existing_accounts.contains_key(&email) {
        todo!("ask user if they want to override data");
    }

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

    let data = StoredAccountData {
        access_token,
        refresh_token,
    };

    existing_accounts.insert(email, data);
    Ok(())
}

/// writes user data to `user.toml` file creating all parent directories in the process
fn store_account_data(
    data: &HashMap<String, StoredAccountData>,
    dir: &PathBuf,
    filename: &str,
) -> anyhow::Result<()> {
    fs::create_dir_all(dir)?;
    fs::write(&dir.join(filename), toml::to_string_pretty(data)?)?;

    Ok(())
}

fn get_data_dir_path() -> anyhow::Result<PathBuf> {
    if let Some(base_dir) = directories::BaseDirs::new() {
        Ok(base_dir.data_dir().join("mail-cli/"))
    } else {
        Err(anyhow!("failed to find home directory"))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = CliArgs::parse();

    match args.command {
        Commands::Login { email } => {
            let path = get_data_dir_path()?;
            let data_str = match fs::read_to_string(&path.join("accounts.toml")) {
                Ok(data) => data,
                Err(err) => match err.kind() {
                    std::io::ErrorKind::NotFound => String::new(),
                    _ => return Err(err.into()),
                },
            };

            let mut existing_accounts: HashMap<String, StoredAccountData> =
                toml::from_str(&data_str)?;

            add_new_account(email, &mut existing_accounts).await?;
            store_account_data(&existing_accounts, &path, "accounts.toml")?;
        }
        Commands::Read { n } => {
            // let accounts = read_account_data(&get_data_dir_path()?.join("accounts.toml"))?;

            // let (
            //     email,
            //     StoredAccountData {
            //         access_token,
            //         refresh_token,
            //     },
            // ): (String, StoredAccountData) = todo!("add function to get account");

            // let imap_auth = ImapOAuth2 {
            //     user: email.clone(),
            //     access_token,
            // };

            // let mut session =
            //     match create_imap_session(GOOGLE_IMAP_DOMAIN, GOOGLE_IMAP_PORT, &imap_auth) {
            //         Ok(session) => session,
            //         Err(_) => {
            //             let GoogleOAuthTokenRefreshResponse { access_token } =
            //                 refresh_google_oauth_token(
            //                     &Client::new(),
            //                     &GoogleOAuthParams::default(),
            //                     &refresh_token,
            //                 )
            //                 .await?;

            //             let data = StoredAccountData {
            //                 access_token: access_token.clone(),
            //                 refresh_token,
            //             };

            //             store_account_data(&data)?;

            //             let imap_auth = ImapOAuth2 {
            //                 user: email,
            //                 access_token,
            //             };
            //             create_imap_session(GOOGLE_IMAP_DOMAIN, GOOGLE_IMAP_PORT, &imap_auth)?
            //         }
            //     };

            // let msg = fetch_top_n_msg_from_inbox(&mut session, n)?;

            // println!("{msg:?}");

            // session.logout()?;
        }
    }

    Ok(())
}
