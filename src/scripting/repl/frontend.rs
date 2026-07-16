use std::io::{BufRead, Write};
use std::sync::mpsc;

pub struct MeowMeowReplFrontend {
    rx: mpsc::Receiver<String>,
}

impl MeowMeowReplFrontend {
    pub fn new() -> Result<Self, &'static str> {
        crate::engine::repl::claim_stdin()?;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let stdin = std::io::stdin();
            let mut buffered = String::new();
            loop {
                print!("{}", if buffered.is_empty() { "mms> " } else { "... " });
                let _ = std::io::stdout().flush();
                let mut line = String::new();
                match stdin.lock().read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                buffered.push_str(&line);
                if input_complete(&buffered) {
                    if tx.send(std::mem::take(&mut buffered)).is_err() {
                        break;
                    }
                }
            }
            crate::engine::repl::release_stdin();
        });
        Ok(Self { rx })
    }

    pub fn try_recv_all(&self) -> Vec<String> {
        let mut out = Vec::new();
        while let Ok(source) = self.rx.try_recv() {
            out.push(source);
        }
        out
    }
}

pub(crate) fn input_complete(source: &str) -> bool {
    let mut stack = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    let mut line_comment = false;
    let chars = source.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        if line_comment {
            if ch == '\n' {
                line_comment = false;
            }
            index += 1;
            continue;
        }
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == q {
                quote = None;
            }
            index += 1;
            continue;
        }
        if ch == '/' && chars.get(index + 1) == Some(&'/') {
            line_comment = true;
            index += 2;
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' | '[' | '{' => stack.push(ch),
            ')' => {
                if stack.last() == Some(&'(') {
                    stack.pop();
                }
            }
            ']' => {
                if stack.last() == Some(&'[') {
                    stack.pop();
                }
            }
            '}' => {
                if stack.last() == Some(&'{') {
                    stack.pop();
                }
            }
            _ => {}
        }
        index += 1;
    }
    quote.is_none() && stack.is_empty()
}

#[cfg(test)]
mod tests {
    use super::input_complete;
    #[test]
    fn balances_multiline_and_quoted_delimiters() {
        assert!(!input_complete("T {\n Text {"));
        assert!(input_complete("T {\n Text { \"}\" }\n}"));
        assert!(!input_complete("\"escaped \\\" quote"));
        assert!(input_complete("query(world, \"#x > Text\")"));
        assert!(input_complete("// an unmatched { in a comment\n42"));
    }
}
