use super::services::{MessageId, UserId};

#[derive(Clone, Default)]
pub struct MessageSettings {
    pub embed: Option<MessageEmbed>,
    pub reply: Option<MessageId>,
    pub reply_user: Option<UserId>,
    pub attachments: Vec<(String, Vec<u8>)>,
}

#[derive(Clone, Default)]
pub struct MessageEmbed {
    pub author_name: Option<String>,
    pub author_icon_url: Option<String>,
    pub author_url: Option<String>,
    pub color: Option<u32>,
    pub description: Option<String>,
    pub fields: Vec<(String, String, bool)>,
    pub footer_text: Option<String>,
    pub footer_icon_url: Option<String>,
    pub image: Option<String>,
    pub thumbnail: Option<String>,
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
    pub title: Option<String>,
    pub attachment: Option<String>,
}

pub enum MessageContent<'a> {
    String(String),
    Str(&'a str),
}

pub struct Attachment {
    pub filename: String,
    pub url: String,
    pub size: Option<u64>,
    pub dimensions: Option<(u64, u64)>,
}

pub trait ToMessageContent<'a>: Send + Sync {
    fn to_message_content(self) -> MessageContent<'a>;
}

impl ToMessageContent<'static> for String {
    fn to_message_content(self) -> MessageContent<'static> {
        MessageContent::String(self)
    }
}

impl<'a> ToMessageContent<'a> for &'a str {
    fn to_message_content(self) -> MessageContent<'a> {
        MessageContent::Str(self)
    }
}
