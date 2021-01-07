use thiserror::Error;

#[derive(Copy, Clone, PartialEq, Eq)]
enum Quote {
    Unquoted,
    SingleQuoted,
    DoubleQuoted,
}

impl Quote {
    pub fn to_str(&self) -> Option<&'static str> {
        match self {
            Quote::Unquoted => None,
            Quote::SingleQuoted => Some("'"),
            Quote::DoubleQuoted => Some("\""),
        }
    }
}

enum ParseState {
    Start,
    Arg(Quote),
    Escape(Quote),
}

pub fn parse_shell_args(text: &str) -> Result<Vec<String>, ArgsError> {
    let mut args = Vec::new();
    let mut arg = String::new();
    let mut state = ParseState::Start;

    let mut chars = text.chars();

    loop {
        let c = chars.next();

        state = match state {
            ParseState::Start => match c {
                None => break,
                Some('\'') => ParseState::Arg(Quote::SingleQuoted),
                Some('\"') => ParseState::Arg(Quote::DoubleQuoted),
                Some('\\') => ParseState::Escape(Quote::Unquoted),
                Some('\t') | Some(' ') => ParseState::Start,
                Some(c) => {
                    arg.push(c);
                    ParseState::Arg(Quote::Unquoted)
                }
            },
            ParseState::Arg(quote) => match c {
                None if quote == Quote::Unquoted => {
                    args.push(std::mem::replace(&mut arg, String::new()));
                    break;
                }
                None => return Err(ArgsError::UnexpectedEof(quote.to_str().unwrap())),
                Some('\'') if quote == Quote::Unquoted => ParseState::Arg(Quote::SingleQuoted),
                Some('\"') if quote == Quote::Unquoted => ParseState::Arg(Quote::DoubleQuoted),
                Some('\'') if quote == Quote::SingleQuoted => ParseState::Arg(Quote::Unquoted),
                Some('\"') if quote == Quote::DoubleQuoted => ParseState::Arg(Quote::Unquoted),
                Some(' ') if quote == Quote::Unquoted => {
                    args.push(std::mem::replace(&mut arg, String::new()));
                    ParseState::Start
                }
                Some('\\') => ParseState::Escape(quote),
                Some(c) => {
                    arg.push(c);
                    ParseState::Arg(quote)
                }
            },
            ParseState::Escape(quote) => match c {
                None => match quote {
                    Quote::Unquoted => {
                        arg.push('\\');
                        args.push(std::mem::replace(&mut arg, String::new()));
                        break;
                    }
                    Quote::SingleQuoted | Quote::DoubleQuoted => {
                        return Err(ArgsError::UnexpectedEof(quote.to_str().unwrap()));
                    }
                },
                Some(c) => {
                    arg.push(c);
                    ParseState::Arg(quote)
                }
            },
        }
    }

    Ok(args)
}

#[derive(Debug, Error)]
pub enum ArgsError {
    #[error("unexpected EOF while looking for matching {_0}")]
    UnexpectedEof(&'static str),
}
