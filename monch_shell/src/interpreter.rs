use crate::builtin::BUILTINS;
use crate::exe::{Execute, Exit, ExternalExecutable, Wait};
use crate::streams::{stream_pipe, ReadStream, Streams, WriteStream};
use crate::types::{can_connect, Ty};
use crate::Error;
use itertools::{izip, Itertools};
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
        let name = self.eval_term(name_term)?;
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
        let name = self.eval_term(name_term)?;
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

        // Evaluate all the command arguments
        let arguments: Vec<Vec<String>> = cmd
            .pipeline
            .iter()
            .map(|inv| {
                inv.arguments
                    .iter()
                    .map(|t| self.eval_term(t))
                    .collect::<Result<_, _>>()
            })
            .collect::<Result<_, _>>()?;

        let arguments_as_ref: Vec<Vec<&str>> = arguments
            .iter()
            .map(|inv| inv.iter().map(String::as_str).collect())
            .collect();

        // Configure all the processes, consuming the IO streams
        let mut executables: Vec<Box<dyn Execute>> = Vec::with_capacity(cmd.pipeline.len());
        for inv in &cmd.pipeline {
            // Evaluate the name of the binary
            let bin_name = self.eval_term(&inv.executable)?;

            // Resolve the executable to an actual thing we can run
            let exe = self.resolve_exe(&bin_name)?;

            executables.push(exe)
        }

        // Type-check the pipeline.
        for ((l_inv, l_exe, l_args), (r_inv, r_exe, r_args)) in
            izip!(&cmd.pipeline, &executables, &arguments_as_ref).tuple_windows()
        {
            let l_output = l_exe.output_type(l_args);
            let r_input = r_exe.input_type(r_args);

            if !can_connect(l_output, r_input) {
                return Err(Error::TypeMismatch {
                    l_cmd: self.eval_term(&l_inv.executable)?,
                    l_ty: l_output,
                    r_cmd: self.eval_term(&r_inv.executable)?,
                    r_ty: r_input,
                });
            }
        }

        // Start all the processes
        let mut children: Vec<Box<dyn Wait>> = Vec::with_capacity(cmd.pipeline.len());
        for (exe, args, streams) in izip!(&executables, &arguments_as_ref, io_streams) {
            // Start up the child proces, with its IO hooked up correctly
            let child = exe.execute(self, streams, args)?;
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

    /// Resolve the name of a command into an Execute impl.
    fn resolve_exe(&self, bin_name: &str) -> Result<Box<dyn Execute>, Error> {
        use std::env;

        // Try to look up a builtin with that name
        if let Some(builtin) = BUILTINS.get(bin_name) {
            return Ok(Box::new(builtin));
        }

        // Try to look up a program on the monch PATH
        match which::which_in(bin_name, env::var_os("MONCH_PATH"), self.current_dir()) {
            Err(e) => match e {
                which::Error::CannotFindBinaryPath => {} // fall through to the other lookups

                // If `which` has some other nasty error, return it.
                _ => {
                    return Err(Error::ResolveBinary {
                        cmd: bin_name.to_string(),
                        source: e,
                    });
                }
            },

            // We found a binary on the MONCH_PATH.
            Ok(monch_bin) => {
                let mut exe = ExternalExecutable::new(monch_bin);

                // Because we found this program on MONCH_PATH, we're expecting CBOR
                exe.set_input_type(Ty::Cbor);
                exe.set_output_type(Ty::Cbor);

                return Ok(Box::new(exe));
            }
        };

        // Try to look up a program on the system PATH
        match which::which_in(bin_name, env::var_os("PATH"), self.current_dir()) {
            Ok(other_bin) => {
                let exe = ExternalExecutable::new(other_bin);
                // input and output types set by default in new()

                Ok(Box::new(exe))
            }

            Err(e) => Err(Error::ResolveBinary {
                cmd: bin_name.to_string(),
                source: e,
            }),
        }
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
        for stream in ios.iter_mut().skip(1) {
            stream.stderr = ends.stderr.try_clone()?; // dup() the stream
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
