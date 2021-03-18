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

pub fn parse_shell_args(markdown: bool, text: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut arg = String::new();
    let mut state = ParseState::Start;

    let mut chars = text.chars().enumerate();

    let mut prev_char = None;

    loop {
        let (c, offset) = match chars.next() {
            Some((offset, c)) => (Some(c), offset),
            None => (None, text.chars().count()),
        };

        state = match state {
            ParseState::Start => match c {
                None => break,
                Some('\'') => ParseState::Arg(Quote::SingleQuoted),
                Some('\"') => ParseState::Arg(Quote::DoubleQuoted),
                Some('\\') => ParseState::Escape(Quote::Unquoted),
                Some('`') if markdown => {
                    if prev_char.is_none() || prev_char == Some('\n') || prev_char == Some(' ') {
                        let rest = &text[offset..];
                        if rest.starts_with("```") {
                            let rest2 = &text[offset + 3..];
                            let is_arg = rest.starts_with("```\n") || rest.starts_with("``` ");
                            if let Some(end) = rest2.find("```").map(|o| o + 3) {
                                if is_arg {
                                    args.push(rest[3..end].into());
                                } else {
                                    args.push(rest[0..end + 3].into());
                                }

                                for _ in 0..(end + 3) {
                                    prev_char = chars.next().map(|(_, c)| c);
                                }

                                state = ParseState::Start;
                                continue;
                            } else {
                                arg.push('`');
                                ParseState::Arg(Quote::Unquoted)
                            }
                        } else {
                            arg.push('`');
                            ParseState::Arg(Quote::Unquoted)
                        }
                    } else {
                        arg.push('`');
                        ParseState::Arg(Quote::Unquoted)
                    }
                }
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
                None => {
                    args.push(format!("{}{}", quote.to_str().unwrap(), arg));
                    break;
                }
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
                        args.push(format!("{}{}\\", quote.to_str().unwrap(), arg));
                        break;
                    }
                },
                Some(c) => {
                    arg.push(c);
                    ParseState::Arg(quote)
                }
            },
        };

        prev_char = c;
    }

    args
}
