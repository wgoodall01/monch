use monch_syntax::Parser;
use rustyline::error::ReadlineError;
use std::env;

pub(crate) mod interpreter;
pub(crate) mod streams;

use interpreter::Interpreter;
use streams::Streams;

fn main() {
    let mut rl = rustyline::Editor::<()>::new();

    let stdio = Streams::stdio().expect("couldn't open stdio");
    let workdir = env::current_dir().expect("bad working directory");

    let mut interpreter = Interpreter::new(stdio, &workdir);
    let parser = Parser::new();

    let mut last_exit_code: i32 = 0;

    loop {
        let prompt = match last_exit_code {
            0 => "monch$ ".to_string(),
            n => format!("[{}] monch$ ", n),
        };

        match rl.readline(&prompt) {
            Ok(line) => {
                // Ignore empty inputs. Technically they don't parse.
                if line.trim().is_empty() {
                    last_exit_code = 0;
                    continue;
                }

                let cmd = match parser.parse_command(&line) {
                    Ok(cmd) => cmd,
                    Err(e) => {
                        println!("monch: {}", e);
                        last_exit_code = 127;
                        continue;
                    }
                };

                last_exit_code = match interpreter.eval_command(&cmd) {
                    Ok(exit) => exit,
                    Err(e) => {
                        println!("monch: {}", e);
                        127
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
