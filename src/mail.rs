use std::{
    fmt::Display,
    net::TcpStream,
    str::from_utf8, collections::HashSet,
};

use anyhow::anyhow;
use imap::Session;
use itertools::Itertools;
use mail_parser::{DateTime, Message};
use native_tls::TlsStream;

use crate::mail_filters::{HeaderField, HeaderFilter};

#[derive(Debug, Clone)]
pub struct Mail {
    #[allow(dead_code)]
    ord_num: u32,
    from: Option<String>,
    to: Option<String>,
    date: Option<DateTime>,
    subject: Option<String>,
    body: String,
}

#[derive(Debug, Clone)]
pub struct MailBox<'a> {
    name: &'a str,
}

impl<'a> MailBox<'a> {
    #[allow(dead_code)]
    pub const INBOX: MailBox<'a> = MailBox { name: "Inbox" };

    pub fn new(name: &'a str) -> Self {
        Self { name }
    }

    pub fn name(&self) -> &str {
        self.name
    }

    pub fn fetch_n_recent_mails(
        &self,
        n: usize,
        session: &mut Session<TlsStream<TcpStream>>,
    ) -> anyhow::Result<Vec<anyhow::Result<Mail>>> {
        session.select(self.name())?;

        let recent_ord_nums = get_mails_sorted_by_date(session)?;
        let fetch_str = recent_ord_nums
            .into_iter()
            .take(n)
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let mailbox_items = session.fetch(&fetch_str, "BODY.PEEK[]")?;
        let mails: Vec<_> = mailbox_items
            .into_iter()
            .map(|item| {
                let msg_str = from_utf8(item.body().unwrap_or(&[])).map(|str| str.to_owned())?;
                let Some(parsed_msg) = Message::parse(msg_str.as_bytes()) else { 
                    return Err(anyhow!("failed to parse mail"))
                };

                Ok(Mail::from_msg(parsed_msg, item.message))
                
            })
            .rev()
            .collect();

        Ok(mails)
    }
}

impl Mail {
    fn from_msg(msg: Message, ord_num: u32) -> Self {
        Self {
            ord_num,
            from: msg.header_raw("from").map(|val| val.to_owned()),
            to: msg.header_raw("to").map(|val| val.to_owned()),
            date: msg.date().cloned(),
            subject: msg.subject().map(|val| val.to_owned()),
            body: msg
                .text_bodies()
                .map(|b| b.text_contents().unwrap())
                .collect::<Vec<_>>()
                .join(""),
        }
    }
}

impl Display for Mail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = format!(
            "From:       {from}
To:         {to}
Send Date:  {date}


Subject:    {sub}

{body}",
            from = self.from.as_ref().map(|val| val.trim()).unwrap_or("-"),
            to = self.to.as_ref().map(|val| val.trim()).unwrap_or("-"),
            date = self
                .date
                .as_ref()
                .map(|date| date.to_string())
                .unwrap_or(String::from("-"))
                .trim(),
            sub = self.subject.as_ref().map(|val| val.trim()).unwrap_or("-"),
            body = self.body.trim()
        );

        write!(f, "{str}")
    }
}

/// returns ordering numbers of all mails in the selected mailbox order by date.
/// the order is descending (newest -> oldest)
fn get_mails_sorted_by_date(session: &mut Session<TlsStream<TcpStream>>) -> anyhow::Result<Vec<u32>> {
    let all_ord_nums = session.search("ALL")?;
    let fetch_str = all_ord_nums.into_iter().join(",");
    let filter_str = HeaderFilter::new(HashSet::from([HeaderField::Date(None),]), false).filter_str().unwrap_or(String::new());

    let recent_ord_nums: Vec<_> = session.fetch(&fetch_str, format!("BODY.PEEK[{filter_str}]"))?
        .into_iter()
        .map(|item| {
            let header_str = from_utf8(item.header().unwrap_or(&[])).unwrap().split_once(":").unwrap().1.trim();
            let date = chrono::DateTime::parse_from_rfc2822(header_str).unwrap();

            (date, item.message)
    })
    .sorted_by(|(date_a, _), (date_b, _)| date_a.cmp(&date_b))
    .rev()
    .map(|(_, num)| num)
    .collect(); 

    Ok(recent_ord_nums)
}
