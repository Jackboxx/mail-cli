use std::{
    fmt::Display,
    net::TcpStream,
    str::from_utf8,
};

use anyhow::anyhow;
use imap::Session;
use mail_parser::{DateTime, Message};
use native_tls::TlsStream;

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

    /// TODO: this has a bug
    /// it does not fetch the latest mails
    ///
    /// Errors:
    /// - todo
    pub fn fetch_n_msgs(
        &self,
        n: u32,
        session: &mut Session<TlsStream<TcpStream>>,
    ) -> anyhow::Result<Vec<anyhow::Result<Mail>>> {
        session.select(self.name())?;

        let fetch_str = (0..n)
            .map(|x| (x + 1).to_string())
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
