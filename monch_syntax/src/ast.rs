use serde::{Deserialize, Serialize};

/// An invocation of a single program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invocation {
    /// The name of the binary we need to run
    pub executable: Term,

    /// The `argv` we want to pass to that binary
    pub arguments: Vec<Term>,
}

/// A complete shell command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    /// The invocations which make up a pipeline, from left to right.
    /// The output of `pipeline[0]` connects to the input of `pipeline[1]`, and so on.
    pub pipeline: Vec<Invocation>,

    /// Optionally, an input redirection (like `cat <file.txt`)
    pub stdin_redirect: Option<ReadRedirect>,

    /// Optionally, an output redirection (like `echo thing >out.txt`)
    pub stdout_redirect: Option<WriteRedirect>,
}

/// A script: for now, just a list of commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Script {
    /// List of commands to execute.
    pub commands: Vec<Command>,
}

/// Places where we can read redirected input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReadRedirect {
    /// Get this process's input from the contents of a file.
    File { file: Term },
}

/// Places where we can write redirected output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WriteRedirect {
    /// Truncate a file, and write the output into it.
    TruncateFile { file: Term },

    /// Append the output to a file.
    AppendFile { file: Term },
}

/// Something which evaluates to a string value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Term {
    /// A literal term
    Literal { value: String },
    // TODO: variable identifiers
    // TODO: format strings (i.e: concatenation of several terms)
}
