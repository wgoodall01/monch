use crate::builtin::BUILTINS;
use crate::exe::{Execute, Exit, ExternalExecutable, Wait};
use crate::streams::{stream_pipe, ReadStream, Streams, WriteStream};
use crate::Error;
use itertools::zip;
use monch_syntax::ast;
use std::fs;
use std::path::{Path, PathBuf};

// TODO: settings, like 'set -e', pipefail, and the like
// TODO: perhaps some kind of mock execution for testing
#[derive(Default)]
pub struct Interpreter {
    /// IO Streams
    ios: Streams,

    /// Current working directory
    current_dir: PathBuf,
}

impl Interpreter {
    /// Create a new Interpreter using the given streams for I/O
    pub fn new(ios: Streams, current_dir: &Path) -> Interpreter {
        Interpreter {
            ios,
            current_dir: current_dir.to_path_buf(),
        }
    }

    /// Get the current working directory of the Interpreter
    pub fn current_dir(&self) -> &Path {
        &self.current_dir
    }

    /// Set the current working directory of the Interpreter
    pub fn set_current_dir(&mut self, new_cwd: impl AsRef<Path>) {
        self.current_dir = new_cwd.as_ref().into();
    }

    /// Evaluate the given command, returning its exit code.
    pub fn eval_command(&mut self, cmd: &ast::Command) -> Result<Exit, Error> {
        let n_invocations = cmd.pipeline.len();

        // Figure out all the io [`Streams`] configuration we're going to need for each process
        let mut ios: Vec<Streams> = Vec::with_capacity(n_invocations);
        for i in 0..n_invocations {
            // We need to special-case the first and last invocation in the pipeline,
            // because their stdio needs to be connected through to the shell's
            let first = i == 0;
            let last = i == n_invocations - 1;

            let inv = &cmd.pipeline[i];

            // ------
            // --- Determine stdout
            let stdout = if let Some(redir) = &inv.stdout_redirect {
                // Options to open any file for writing
                let mut opts = fs::OpenOptions::new();
                opts.write(true);
                opts.create(true);

                // Figure out the filename, and if we're appending or not.
                let name_term = match redir {
                    ast::WriteRedirect::TruncateFile { file } => {
                        opts.truncate(true);
                        file
                    }
                    ast::WriteRedirect::AppendFile { file } => {
                        opts.append(true);
                        file
                    }
                };

                // Open the file.
                let name = self.eval_term(name_term)?;
                let path: PathBuf = self.current_dir.join(name);
                let file = opts.open(&path)?;
                WriteStream::File(file)
            } else if last {
                // Here, we're last in the pipeline---connect to the stdout of the interpreter
                self.ios.stdout.try_clone()?
            } else {
                // If we're not redirected, or last, our output is either:
                //  - piped into the next process, which will get taken care of on the next loop
                //    iteration, or
                //  - ignored, so we let the Null stand
                WriteStream::Null
            };

            // ------
            // --- Determine stdin
            let stdin = if let Some(redir) = &inv.stdin_redirect {
                // Here, we have our input redirected. Open the file and connect that.
                let name_term = match redir {
                    ast::ReadRedirect::File { file } => file,
                };
                let name = self.eval_term(name_term)?;
                let path: PathBuf = self.current_dir.join(name);
                let file = fs::File::open(&path)?;
                ReadStream::File(file)
            } else if first {
                // Here, we're first in the pipeline, with no redirects.
                // Connect stdin to the parent interpreter.
                self.ios.stdin.try_clone()?
            } else if cmd.pipeline[i - 1].stdout_redirect.is_none() {
                //                 ^ never panics, we're not first

                // If the last command didn't have its output redirected, we need to connect it to
                // the input of this process.

                // Create an OS pipe between processes
                let (read, write) = stream_pipe()?;

                // Give the output of the last process the write half
                ios[i - 1].stdout = write;

                // Give the input of this process the read half
                read
            } else {
                // Here, the previous process's input was redirected somewhere.
                // We have no input---so we use the null readstream.
                ReadStream::Null
            };

            // Stderr is (for now) not redirected
            let stderr = self.ios.stderr.try_clone()?;

            // Add the stream configuration to the list
            ios.push(Streams {
                stdin,
                stdout,
                stderr,
            });
        }

        // Configure all the processes, consuming the IO streams
        let mut children: Vec<Box<dyn Wait>> = Vec::with_capacity(n_invocations);
        for (inv, ios) in zip(&cmd.pipeline, ios.into_iter()) {
            // Evaluate all the arguments
            let args: Vec<String> = inv
                .arguments
                .iter()
                .map(|t| self.eval_term(t))
                .collect::<Result<_, _>>()?;

            // Get the args as an &[&str]
            let args_ref: &[&str] = &args.iter().map(String::as_str).collect::<Vec<_>>();

            // Evaluate the name of the binary
            let bin_name = self.eval_term(&inv.executable)?;

            // Look up the binary name in the set of builtins
            // Note: this lookup is case-sensitive.
            let boxed_exe: Box<dyn Execute>; // place to own new [`Execute`]s if we make them
            let exe: &dyn Execute = if let Some(builtin) = BUILTINS.get(bin_name.as_str()) {
                // We're running an in-process builtin
                *builtin
            } else {
                // We're running an external program
                boxed_exe = Box::new(ExternalExecutable(bin_name));
                &*boxed_exe
            };

            // Start up the child proces, with its IO hooked up correctly
            let child = exe.execute(self, ios, args_ref)?;

            children.push(child);
        }

        // Wait for all the child processes to finish
        let exit_codes: Vec<Exit> = children
            .into_iter()
            .map(|c| c.wait())
            .collect::<Result<_, Error>>()?;

        // Come up with an exit status that represents the entire pipeline.
        // We use Bash's `&&` logic here by default.
        let exit = exit_codes
            .into_iter()
            .reduce(Exit::reduce_worst) // Pick the worst of their exit codes
            .unwrap_or(Exit::SUCCESS); // If we don't have any children, return success.

        Ok(exit)
    }

    /// Evaluate an [`ast::Term`] to a [`String`] value
    pub fn eval_term(&self, term: &ast::Term) -> Result<String, Error> {
        match term {
            ast::Term::Literal { value } => Ok(value.clone()),
        }
    }
}
