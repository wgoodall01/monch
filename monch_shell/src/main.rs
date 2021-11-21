use monch_syntax::Parser;
use rustyline::error::ReadlineError;
use std::env;

pub(crate) mod builtin;
pub(crate) mod exe;
pub(crate) mod interpreter;
pub(crate) mod streams;

mod error;
pub use error::Error;

use exe::Exit;
use interpreter::Interpreter;
use streams::Streams;

fn main() {
    let mut rl = rustyline::Editor::<()>::new();

    let stdio = Streams::stdio().expect("couldn't open stdio");
    let workdir = env::current_dir().expect("bad working directory");

    let mut interpreter = Interpreter::new(stdio, &workdir);
    let parser = Parser::new();

    // Keep track of the last exit code we've seen
    let mut last_exit = Exit::SUCCESS;

    loop {
        let prompt = if last_exit.success() {
            "monch $ ".to_string()
        } else {
            format!("[{}] monch $ ", last_exit)
        };

        match rl.readline(&prompt) {
            Ok(line) => {
                // Ignore empty inputs. Technically they don't parse.
                if line.trim().is_empty() {
                    last_exit = Exit::SUCCESS;
                    continue;
                }

                // Parse the command line
                let cmd = match parser.parse_command(&line) {
                    Ok(cmd) => cmd,

                    // Handle parse errors by printing them, setting last_exit, and skipping
                    // evaluation.
                    Err(e) => {
                        println!("monch: {}", e);
                        last_exit = Exit::BAD_SYNTAX;
                        continue;
                    }
                };

                // Evaluate the command line AST
                last_exit = match interpreter.eval_command(&cmd) {
                    Ok(exit) => exit,

                    // Handle errors by printing them and setting last_exit.
                    Err(e) => {
                        println!("monch: {}", e);
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
                println!("error reading line: {:?}", err);
                break;
            }
        }
    }
}
