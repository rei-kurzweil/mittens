use std::io::BufRead;
use std::sync::mpsc;

/// Simple stdin-driven REPL.
///
/// This spawns a background thread that blocks on stdin and forwards each line
/// to the main thread over a channel.
pub struct Repl {
    rx: mpsc::Receiver<String>,
    _thread: std::thread::JoinHandle<()>,
}

impl Repl {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<String>();

        let handle = std::thread::spawn(move || {
            let stdin = std::io::stdin();
            for line in stdin.lock().lines() {
                match line {
                    Ok(cmd) => {
                        if tx.send(cmd).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            rx,
            _thread: handle,
        }
    }

    /// Drain all currently queued commands without blocking.
    pub fn try_recv_all(&self) -> Vec<String> {
        let mut out = Vec::new();
        while let Ok(cmd) = self.rx.try_recv() {
            out.push(cmd);
        }
        out
    }
}
