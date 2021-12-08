use monch_syntax::Parser;
use owo_colors::OwoColorize;
use rustyline::completion::Completer;
use rustyline::completion::FilenameCompleter;
use rustyline::completion::Pair;
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::line_buffer::LineBuffer;
use rustyline::validate::Validator;
use rustyline::Context;
use std::env;

use monch_shell::{Exit, Interpreter, Streams};

fn main() {
    #[cfg(debug_assertions)]
    {
        // Find the directory the current executable is stored in
        let this_exe = env::current_exe().expect("couldn't get path to monch executable");
        let monch_path = this_exe
            .parent()
            .expect("this executable must have a parent dir")
            .canonicalize()
            .expect("the parent dir of this executable must be canonicalizable");

        // If we're a debug build, add the Cargo target directory to MONCH_PATH.
        env::set_var("MONCH_PATH", monch_path);
    }

    // Set up readline
    let mut rl = rustyline::Editor::new();
    rl.set_helper(Some(Helper::new()));

    // Set up the interpreter
    let stdio = Streams::stdio().expect("couldn't open stdio");
    let workdir = env::current_dir().expect("bad working directory");
    let mut interpreter = Interpreter::new(stdio, &workdir);

    // Make a parser
    let parser = Parser::new();

    // Keep track of the last exit code we've seen
    let mut last_exit = Exit::SUCCESS;

    loop {
        match rl.readline(&prompt(&interpreter, last_exit)) {
            Ok(line) => {
                // Ignore empty inputs. Technically they don't parse.
                if line.trim().is_empty() {
                    last_exit = Exit::SUCCESS;
                    continue;
                }

                // Add the line as a history entry
                rl.add_history_entry(&line);

                // Parse the command line
                let cmd = match parser.parse_command(&line) {
                    Ok(cmd) => cmd,

                    // Handle parse errors by printing them, setting last_exit, and skipping
                    // evaluation.
                    Err(e) => {
                        eprintln!("monch: {}", e);
                        last_exit = Exit::BAD_SYNTAX;
                        continue;
                    }
                };

                // Evaluate the command line AST
                last_exit = match interpreter.eval_command(&cmd) {
                    Ok(exit) => exit,

                    // Handle errors by printing them and setting last_exit.
                    Err(e) => {
                        eprintln!("monch: {}", e);
                        e.as_exit()
                    }
                };
            }
            Err(ReadlineError::Interrupted) => {
                continue;
            }
            Err(ReadlineError::Eof) => {
                break;
            }
            Err(err) => {
                eprintln!("error reading line: {:?}", err);
                break;
            }
        }

        // Update the actual working directory of this process to that of the interpreter
        if let Err(e) = env::set_current_dir(interpreter.current_dir()) {
            eprintln!("monch: could not update working directory: {}", e);
        }
    }
}

struct Helper {
    completer: FilenameCompleter,
}

impl Helper {
    fn new() -> Helper {
        Helper {
            completer: FilenameCompleter::new(),
        }
    }
}

impl rustyline::Helper for Helper {}

impl Completer for Helper {
    type Candidate = Pair;
    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        self.completer.complete(line, pos, ctx)
    }

    fn update(&self, line: &mut LineBuffer, start: usize, elected: &str) {
        self.completer.update(line, start, elected)
    }
}

impl Hinter for Helper {
    type Hint = String;
}

impl Highlighter for Helper {}

impl Validator for Helper {}

/// Generate a shell prompt
fn prompt(int: &Interpreter, last_exit: Exit) -> String {
    // Generate the path segment
    let cwd = int.current_dir();
    let path_segment = cwd.to_string_lossy();

    // Generate the error segment
    let error_segment = if !last_exit.success() {
        format!(" [{}]", last_exit)
    } else {
        "".to_string()
    };

    format!(
        "{}{}{}",
        path_segment.dimmed(),
        error_segment.red(),
        " $ ".purple().bold()
    )
}
