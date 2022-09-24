use crate::services::ServiceKind;

pub mod shell_parser;

pub fn escape_untrusted_text(service: ServiceKind, text: String) -> String {
    match service {
        ServiceKind::Discord => text
            .replace("@everyone", "@\u{200B}everyone")
            .replace("@here", "@\u{200B}here")
            .to_string(),
        #[allow(unreachable_patterns)]
        _ => text,
    }
}

/// Case-insensitive Regex
macro_rules! ci_regex {
    ($regex:literal) => {
        regex::RegexBuilder::new($regex)
            .case_insensitive(true)
            .build()
    };
}
pub(crate) use ci_regex;
