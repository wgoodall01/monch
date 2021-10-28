use os_pipe::{dup_stderr, dup_stdin, dup_stdout, PipeReader, PipeWriter};
use std::{fs, io, process};

/// Represents a readable stream.
#[derive(Debug)]
pub enum ReadStream {
    /// Read from an OS pipe
    Pipe(PipeReader),

    /// Read from an open file
    File(fs::File),

    /// Never read any data.
    Null,
}

impl ReadStream {
    /// Create a new [`ReadStream`] pointing to this process's standard input.
    pub fn stdin() -> io::Result<ReadStream> {
        Ok(ReadStream::Pipe(dup_stdin()?))
    }

    /// Try to clone this ReadStream
    pub fn try_clone(&self) -> io::Result<Self> {
        match self {
            ReadStream::Pipe(p) => Ok(ReadStream::Pipe(p.try_clone()?)),
            ReadStream::File(f) => Ok(ReadStream::File(f.try_clone()?)),
            ReadStream::Null => Ok(ReadStream::Null),
        }
    }
}

impl io::Read for ReadStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            // Return no data
            ReadStream::Null => Ok(0),

            // Pass reads through
            ReadStream::Pipe(r) => r.read(buf),
            ReadStream::File(r) => r.read(buf),
        }
    }
}

impl From<ReadStream> for process::Stdio {
    fn from(rs: ReadStream) -> process::Stdio {
        match rs {
            ReadStream::Pipe(p) => p.into(),
            ReadStream::File(f) => f.into(),
            ReadStream::Null => process::Stdio::null(),
        }
    }
}

/// Represents a writable stream.
#[derive(Debug)]
pub enum WriteStream {
    /// Write to an OS pipe
    Pipe(PipeWriter),

    /// Write into an open file
    File(fs::File),

    /// Discard all data written
    Null,
}

impl WriteStream {
    /// Create a new [`WriteStream`] pointing to this process's standard output.
    pub fn stdout() -> io::Result<WriteStream> {
        Ok(WriteStream::Pipe(dup_stdout()?))
    }

    /// Create a new [`WriteStream`] pointing to this process's standard error.
    pub fn stderr() -> io::Result<WriteStream> {
        Ok(WriteStream::Pipe(dup_stderr()?))
    }

    /// Try to clone this WriteStream
    pub fn try_clone(&self) -> io::Result<Self> {
        match self {
            WriteStream::Pipe(p) => Ok(WriteStream::Pipe(p.try_clone()?)),
            WriteStream::File(f) => Ok(WriteStream::File(f.try_clone()?)),
            WriteStream::Null => Ok(WriteStream::Null),
        }
    }
}

impl io::Write for WriteStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            // Indicate that the write stream is closed for Nulls
            WriteStream::Null => Ok(0),

            // Pass writes through
            WriteStream::Pipe(w) => w.write(buf),
            WriteStream::File(w) => w.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            // Ignore flushes
            WriteStream::Null => Ok(()),

            // Flush the underlying stream
            WriteStream::Pipe(w) => w.flush(),
            WriteStream::File(w) => w.flush(),
        }
    }
}

impl From<WriteStream> for process::Stdio {
    fn from(rs: WriteStream) -> process::Stdio {
        match rs {
            WriteStream::Pipe(p) => p.into(),
            WriteStream::File(f) => f.into(),
            WriteStream::Null => process::Stdio::null(),
        }
    }
}

/// Data streams for stdin, stdout, and stderr.
pub struct Streams {
    pub stdin: ReadStream,
    pub stdout: WriteStream,
    pub stderr: WriteStream,
}

impl Streams {
    /// Create a [`Streams`] that ignores all reads and writes.
    pub fn null() -> Streams {
        Streams {
            stdin: ReadStream::Null,
            stdout: WriteStream::Null,
            stderr: WriteStream::Null,
        }
    }

    /// Create a [`Streams`] connected to this process's standard IO streams.
    pub fn stdio() -> io::Result<Streams> {
        Ok(Streams {
            stdin: ReadStream::stdin()?,
            stdout: WriteStream::stdout()?,
            stderr: WriteStream::stderr()?,
        })
    }
}

impl Default for Streams {
    fn default() -> Streams {
        Streams::null()
    }
}

/// Create an OS pipe, returning the read half and write half, respectively.
pub fn stream_pipe() -> io::Result<(ReadStream, WriteStream)> {
    let (reader, writer) = os_pipe::pipe()?;
    Ok((ReadStream::Pipe(reader), WriteStream::Pipe(writer)))
}
