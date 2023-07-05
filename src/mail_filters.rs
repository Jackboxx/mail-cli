use std::collections::HashSet;

use chrono::{DateTime, FixedOffset};

pub struct HeaderFilter {
    fields: HashSet<HeaderField>,
    negated: bool,
}

#[derive(Debug, Clone, Hash, Eq)]
#[allow(dead_code)]
pub enum HeaderField {
    Subject(Option<String>),
    To(Option<String>),
    From(Option<String>),
    Date(Option<DateTime<FixedOffset>>),
}

impl PartialEq for HeaderField {
    fn eq(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    fn ne(&self, other: &Self) -> bool {
        std::mem::discriminant(self) != std::mem::discriminant(other)
    }
}

impl HeaderFilter {
    pub fn new(fields: HashSet<HeaderField>, negated: bool) -> Self {
        Self { fields, negated }
    }
    /// converts the filter into a valid IMAP filter.
    ///
    /// returns none if `fields` is empty
    pub fn filter_str(&self) -> Option<String> {
        if self.fields.is_empty() {
            return None;
        }

        let negated = if self.negated { ".NOT" } else { "" };
        let fields = self
            .fields
            .iter()
            .map(|field| field.filter_str())
            .collect::<Vec<_>>()
            .join(", ");

        Some(format!("HEADER.FIELDS{negated} ({fields})"))
    }
}

impl HeaderField {
    pub fn filter_str(&self) -> String {
        match self {
            HeaderField::Subject(subject) => {
                format!("SUBJECT {}", subject.as_ref().unwrap_or(&String::new()))
            }
            HeaderField::To(to) => {
                format!("TO {}", to.as_ref().unwrap_or(&String::new()))
            }
            HeaderField::From(from) => {
                format!("TO {}", from.as_ref().unwrap_or(&String::new()))
            }
            HeaderField::Date(date) => {
                format!(
                    "DATE {}",
                    date.map(|date| date.to_rfc2822()).unwrap_or(String::new())
                )
            }
        }
    }
}
