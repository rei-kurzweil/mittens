//! Command-line interface for little-cat.

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
}

impl CLI {
    /// Parse command-line arguments.
    ///
    /// Supported commands:
    /// - `./little-cat save <filename>` - Save the current scene
    /// - `./little-cat load <filename>` - Load a scene from file
    /// - `./little-cat` (no args) - Run normally
    pub fn parse() -> Self {
        let args: Vec<String> = env::args().collect();

        let command = if args.len() >= 3 {
            match args[1].as_str() {
                "save" => CliCommand::Save {
                    filename: args[2].clone(),
                },
                "load" => CliCommand::Load {
                    filename: args[2].clone(),
                },
                _ => {
                    eprintln!("Unknown command: {}. Running normally.", args[1]);
                    CliCommand::Run
                }
            }
        } else {
            CliCommand::Run
        };

        CLI { command }
    }
}
