use crate::services::ServiceKind;

pub mod shell_parser;

pub fn escape_untrusted_text(service: ServiceKind, text: String) -> String {
    match service {
        ServiceKind::Discord => text.replace('@', "@\u{200B}").to_string(),
        #[allow(unreachable_patterns)]
        _ => text,
    }
}
