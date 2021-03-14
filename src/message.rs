use super::services::{MessageId, UserId};

#[derive(Clone, Default)]
pub struct MessageSettings {
    pub reply: Option<MessageId>,
    pub reply_user: Option<UserId>,
}

pub enum MessageContent<'a> {
    String(String),
    Str(&'a str),
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
