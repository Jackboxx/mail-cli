use std::fmt::Display;

use mail_parser::{DateTime, Message};

#[derive(Debug, Clone)]
pub struct Mail {
    from: Option<String>,
    to: Option<String>,
    date: Option<DateTime>,
    subject: Option<String>,
    body: String,
}

impl<'a> From<Message<'a>> for Mail {
    fn from(value: Message) -> Self {
        Self {
            from: value.header_raw("from").map(|val| val.to_owned()),
            to: value.header_raw("to").map(|val| val.to_owned()),
            date: value.date().cloned(),
            subject: value.subject().map(|val| val.to_owned()),
            body: value
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
