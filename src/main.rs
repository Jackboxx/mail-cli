use std::{fmt::Display, net::TcpStream};

use anyhow::anyhow;
use clap::Parser;
use cli::{add_new_account, select_account, CliArgs, Commands};
use colored::Colorize;
use imap::Session;
use mail::MailBox;
use native_tls::TlsStream;
use reqwest::Client;
use store_accounts::{StoredAccountData, StoredAccounts};

use crate::google::{
    refresh_google_oauth_token, GoogleOAuthParams, GoogleOAuthTokenRefreshResponse,
    GOOGLE_IMAP_DOMAIN, GOOGLE_IMAP_PORT,
};

extern crate imap;
extern crate native_tls;
extern crate rpassword;

mod cli;
mod google;
mod mail;
mod store_accounts;
mod utils;

struct ImapOAuth2Data {
    user: String,
    access_token: String,
}

impl imap::Authenticator for ImapOAuth2Data {
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
    imap_auth: &ImapOAuth2Data,
) -> anyhow::Result<Session<TlsStream<TcpStream>>> {
    let tls = native_tls::TlsConnector::builder().build()?;
    let client = imap::connect((domain, port), domain, &tls)?;

    client
        .authenticate("XOAUTH2", imap_auth)
        .map_err(|err| anyhow!(format!("{err:?}")))
}

/// tries to create a session with the given credentials.
/// if it fails to create a session tries to use the refresh token to acquire a new access
/// token and updates the stored account data if it succeeds.
///
/// Errors:
/// - if it fails to retrieve new authentication parameters with the provided refresh token
/// - if it fails to store the new access token to the file system after a successful refresh
/// - if the creation of an IMAP session fails after acquiring and storing a new access token
/// (due to a network error or other cause)
async fn create_imap_session_with_refresh_on_err(
    domain: &str,
    port: u16,
    imap_auth: &ImapOAuth2Data,
    refresh_token: &str,
    email: String,
    accounts: &mut StoredAccounts,
) -> anyhow::Result<Session<TlsStream<TcpStream>>> {
    match create_imap_session(domain, port, imap_auth) {
        Ok(session) => Ok(session),
        Err(_) => {
            let GoogleOAuthTokenRefreshResponse { access_token } = refresh_google_oauth_token(
                &Client::new(),
                &GoogleOAuthParams::default(),
                refresh_token,
            )
            .await?;

            accounts.insert(
                email.clone(),
                StoredAccountData::new(access_token.clone(), refresh_token.to_owned()),
            )?;

            let imap_auth = ImapOAuth2Data {
                user: email,
                access_token,
            };

            create_imap_session(GOOGLE_IMAP_DOMAIN, GOOGLE_IMAP_PORT, &imap_auth)
        }
    }
}

fn print_info<D: Display>(str: D) {
    println!("{i} {str}", i = String::from("!").blue())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv()?;
    let args = CliArgs::parse();

    match args.command {
        Commands::Login { email } => {
            let mut existing_accounts = StoredAccounts::load_data()?;
            add_new_account(email, &mut existing_accounts).await?;
        }
        Commands::Read { n, mailbox, mail } => {
            let mut accounts = StoredAccounts::load_data()?;
            let account = match mail {
                Some(mail) => match accounts.map().get(&mail) {
                    Some(data) => (mail, data.to_owned()),
                    None => {
                        print_info(format!("no account with mail '{mail}' found"));
                        select_account(accounts.map()).ok_or(anyhow!("no account selected"))?
                    }
                },
                None => select_account(accounts.map()).ok_or(anyhow!("no account selected"))?,
            };

            let (
                email,
                StoredAccountData {
                    access_token,
                    refresh_token,
                },
            ) = account;

            let imap_auth = ImapOAuth2Data {
                user: email.clone(),
                access_token,
            };

            let mut session = create_imap_session_with_refresh_on_err(
                GOOGLE_IMAP_DOMAIN,
                GOOGLE_IMAP_PORT,
                &imap_auth,
                &refresh_token,
                email,
                &mut accounts,
            )
            .await?;

            let mailbox = MailBox::new(&mailbox);
            let mails = mailbox.fetch_n_msgs(n, &mut session)?;

            for mail in mails {
                let mail = mail?;
                println!("{mail}");
            }

            session.logout()?;
        }
    }

    Ok(())
}
