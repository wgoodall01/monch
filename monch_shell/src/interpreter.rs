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

    /// Set the current working directory of the Interpreter.
    /// This will fail if we're given a path that cannot be canonicalized, or a path that is not a
    /// directory.
    pub fn set_current_dir(&mut self, new_cwd: impl AsRef<Path>) -> Result<(), Error> {
        // Canonicalize the path
        let new_cwd = new_cwd.as_ref().canonicalize()?;

        // Check our working directory is actually a directory
        if !new_cwd.is_dir() {
            return Err(Error::BadWorkingDirectory(
                new_cwd.to_string_lossy().to_string(),
            ));
        }

        // Update the cwd
        self.current_dir = new_cwd;

        Ok(())
    }

    /// Open a file for input redirection, returning the right ReadStream
    fn eval_read_redirect(&self, redir: &ast::ReadRedirect) -> Result<ReadStream, Error> {
        // Here, we have our input redirected. Open the file and connect that.
        let name_term = match redir {
            ast::ReadRedirect::File { file } => file,
        };
        let name = self.eval_term(&name_term)?;
        let path: PathBuf = self.current_dir.join(name);
        let file = fs::File::open(&path)?;
        Ok(ReadStream::File(file))
    }

    /// Open a file for output redirection, returning the right WriteStream
    fn eval_write_redirect(&self, redir: &ast::WriteRedirect) -> Result<WriteStream, Error> {
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
        let name = self.eval_term(&name_term)?;
        let path: PathBuf = self.current_dir.join(name);
        let file = opts.open(&path)?;
        Ok(WriteStream::File(file))
    }

    /// Evaluate the given command, returning its exit code.
    pub fn eval_command(&mut self, cmd: &ast::Command) -> Result<Exit, Error> {
        // Figure out the stdin for the left end of the pipeline
        let cmd_stdin = match &cmd.stdin_redirect {
            Some(redir) => self.eval_read_redirect(redir)?,
            None => self.ios.stdin.try_clone()?, // If not redirected, inherit from the parent.
        };

        // Figure out the stdout for the right end of the pipeline
        let cmd_stdout = match &cmd.stdout_redirect {
            Some(redir) => self.eval_write_redirect(redir)?,
            None => self.ios.stdout.try_clone()?, // If not redirected, inherit from the parent.
        };

        // The IO streams for the pipeline as a whole
        let pipeline_ends = Streams {
            stdin: cmd_stdin,
            stdout: cmd_stdout,
            stderr: self.ios.stderr.try_clone()?, // always passed through to parent
        };

        // Set up all the plumbing we're going to need to connect processes in the pipeline
        // together.
        let io_streams: Vec<Streams> = self.make_stream_chain(pipeline_ends, cmd.pipeline.len())?;

        // Configure all the processes, consuming the IO streams
        let mut children: Vec<Box<dyn Wait>> = Vec::with_capacity(cmd.pipeline.len());
        for (inv, streams) in zip(&cmd.pipeline, io_streams.into_iter()) {
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
            let child = exe.execute(self, streams, args_ref)?;

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

    /// Create a series of `length` [`Streams`] instances in a (stdout -> stdin) chain.
    fn make_stream_chain(&self, ends: Streams, length: usize) -> Result<Vec<Streams>, Error> {
        assert!(length > 0, "cannot make stream chain with length <= 1");

        // Start with a bunch of null streams, one for each item in the pipeline.
        let mut ios: Vec<Streams> = (0..length).map(|_| Streams::null()).collect();

        // Make a bunch of pipes between stages, and attach them
        for i in 1..length {
            // Connect a pipe to the previous link
            let (read, write) = stream_pipe()?;

            // Give this process the read half
            ios[i].stdin = read;

            // Give the previous process in the chain the write half
            // (note: i in 1..length, skipping first element)
            ios[i - 1].stdout = write;
        }

        // Make a bunch of stderr clones, and attach them to every stage
        for i in 1..length {
            ios[i].stderr = ends.stderr.try_clone()?; // dup() the stream
        }
        ios[0].stderr = ends.stderr; // move the stream, avoiding extra dup()

        // Connect stdin to the first element
        ios[0].stdin = ends.stdin;

        // Connect stdout to the last element
        ios[length - 1].stdout = ends.stdout;

        // Return the list of stream configurations
        Ok(ios)
    }
}
