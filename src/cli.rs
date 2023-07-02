use std::collections::HashMap;

use clap::{Parser, Subcommand};
use dialoguer::{theme::ColorfulTheme, Completion, Input, Select};
use reqwest::Client;

use crate::{
    google::{request_google_oauth_token, GoogleOAuthParams, GoogleOAuthTokenRequestResponse},
    store_accounts::{StoredAccountData, StoredAccounts},
};

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(about = "login to mail account")]
    Login {
        /// the mail address of the account you want to login to
        email: String,
    },
    #[command(about = "read mails")]
    Read {
        /// number of mails to read
        n: u32,
        /// optional mail, if not set you will be prompted to select from the list of logged in
        /// accounts
        /// if the mail you selected is not a logged in account the program will exist with a
        /// failure
        #[arg(short, long)]
        mail: Option<String>,
        #[arg(short = 'b', long, default_value = "INBOX")]
        /// the mailbox to read from
        mailbox: String,
    },
}

pub struct CompletionOptions<'a>(Vec<&'a str>);

impl<'a> Completion for CompletionOptions<'a> {
    fn get(&self, input: &str) -> Option<String> {
        let matches = self
            .0
            .iter()
            .filter(|option| option.starts_with(input))
            .collect::<Vec<_>>();

        if matches.len() == 1 {
            Some(matches[0].to_string())
        } else {
            None
        }
    }
}

/// at the moment this function creates its own client and auth parameters (specifically for
/// google/gmail), in the future when there are multiple email providers supported these should
/// be passed in as function parameters
pub async fn add_new_account(email: String, accounts: &mut StoredAccounts) -> anyhow::Result<()> {
    if accounts.map().contains_key(&email) {
        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "do you want to override the exisiting data for the email {email}",
            ))
            .default(0)
            .items(&["yes", "no"])
            .interact()?;

        if selection == 1 {
            println!("login canceled");
            return Ok(());
        }
    }

    let client = Client::new();
    let auth_params = GoogleOAuthParams::default();

    let code = Input::<String>::with_theme(&ColorfulTheme::default())
        .with_prompt(format!(
            "visit this link: {url}\nand paste the code from it here",
            url = auth_params.get_token_request_url()
        ))
        .interact_text()?;

    let GoogleOAuthTokenRequestResponse {
        access_token,
        refresh_token,
    } = request_google_oauth_token(&client, &auth_params, &code).await?;

    accounts.insert(email, StoredAccountData::new(access_token, refresh_token))
}

pub fn select_account(
    accounts: &HashMap<String, StoredAccountData>,
) -> Option<(String, StoredAccountData)> {
    if accounts.is_empty() {
        None
    } else if accounts.len() == 1 {
        accounts
            .iter()
            .next()
            .map(|(email, data)| (email.to_owned(), data.to_owned()))
    } else {
        let mails: Vec<_> = accounts.keys().map(|key| key.as_str()).collect();
        let prompt = format!(
            "choose an account from the list\n{list}\n",
            list = mails
                .iter()
                .map(|mail| format!("- {mail}"))
                .collect::<Vec<_>>()
                .join("\n")
        );

        let completion = CompletionOptions(mails);
        let picked = match Input::<String>::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .completion_with(&completion)
            .interact_text()
            .ok()
        {
            Some(str) => str,
            None => return None,
        };

        accounts.get(&picked).map(|data| (picked, data.to_owned()))
    }
}
