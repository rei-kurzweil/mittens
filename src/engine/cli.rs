//! Command-line interface for mittens-engine.

use crate::engine::graphics::MsaaMode;
use std::env;

#[derive(Debug, Clone, PartialEq)]
pub enum CliCommand {
    /// Save the scene to a file.
    Save { filename: String },
    /// Load a scene from a file.
    Load { filename: String },
    /// Run normally (no special command).
    Run,
}

pub struct CLI {
    pub command: CliCommand,
    pub msaa_mode: Option<MsaaMode>,
}

impl CLI {
    /// Parse command-line arguments.
    ///
    /// Supported commands:
    /// - `./mittens-engine save <filename>` - Save the current scene
    /// - `./mittens-engine load <filename>` - Load a scene from file
    /// - `./mittens-engine` (no args) - Run normally
    pub fn parse() -> Self {
        let args: Vec<String> = env::args().skip(1).collect();

        let mut msaa_mode: Option<MsaaMode> = None;
        let mut positional: Vec<String> = Vec::new();

        for arg in args {
            match arg.as_str() {
                "--no-msaa" | "--msaa=off" => msaa_mode = Some(MsaaMode::Off),
                "--msaa4x" | "--msaa=4x" => msaa_mode = Some(MsaaMode::Msaa4x),
                _ if arg.starts_with("--") => {
                    eprintln!("Unknown flag: {arg}");
                }
                _ => positional.push(arg),
            }
        }

        let command = match positional.as_slice() {
            [cmd, filename] if cmd == "save" => CliCommand::Save {
                filename: filename.to_string(),
            },
            [cmd, filename] if cmd == "load" => CliCommand::Load {
                filename: filename.to_string(),
            },
            [] => CliCommand::Run,
            [unknown, ..] => {
                eprintln!("Unknown command: {unknown}. Running normally.");
                CliCommand::Run
            }
        };

        CLI { command, msaa_mode }
    }
}
