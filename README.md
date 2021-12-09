# Monch

A Structured Shell

## Structure

Monch is split up into a few different crates:

- `monch_shell`: The shell itself. Provides the `monch` binary.
- `monch_io`: A set of utilities for the shell and `monch`-compatible programs to read and write objects from stdin and stdout
- `monch_syntax`: The shell's parser and grammar definition.
- `monch_util_*`: Utilities that work well with `monch`
  - `get`: Extract a value from a stream of objects by its path (similar to `jq`)
  - `grep`: Filter a stream of objects by string matching (optionally on a nested field)
  - `ls`: List files and their metadata
  - `sed`: Replace text in a stream of strings

## Building Monch

If you don't have `cargo` or a Rust toolchain installed already, install `rustup`:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# ...and follow the prompts
```

Then, to build the shell, all its dependencies, and all the utility binaries, run:

```
cargo build
```

To run the shell, run:

```sh
cargo run --bin monch
# (or ./target/{debug,release}/monch, depending on build profile)
```

And to run the tests, run:

```sh
cargo test
```

## Examples

You can run any command on your system in `monch`, and you'll get the output in your shell just like you normally would:

```sh
/ $ ps
    PID TTY          TIME CMD
 292799 pts/1    00:00:00 bash
 292837 pts/1    00:00:00 monch
 293052 pts/1    00:00:00 ps
```

However, if you run a Monch-aware command (like our re-implentation of `ls`, for example), you'll see structured output instead:

```sh
/ $ ls -la
{name: etc, kind: Dir}
{name: mnt, kind: Dir}
{name: run, kind: Dir}
{name: var, kind: Dir}
{name: cdrom, kind: Dir}
{name: usr, kind: Dir}
{name: lib, kind: Unknown}
{name: srv, kind: Dir}
{name: root, kind: Dir}
{name: media, kind: Dir}
{name: snap, kind: Dir}
{name: sys, kind: Dir}
{name: boot, kind: Dir}
{name: opt, kind: Dir}
{name: lib32, kind: Unknown}
{name: home, kind: Dir}
{name: libx32, kind: Unknown}
{name: dev, kind: Dir}
{name: proc, kind: Dir}
{name: lib64, kind: Unknown}
{name: sbin, kind: Unknown}
{name: swapfile, kind: File}
{name: bin, kind: Unknown}
{name: tmp, kind: Dir}
```

You can manipulate streams of objects:

- Use the `get` command to extract a field
- Use the `grep` command to filter by a string field's contents

So, to get a list of only the directories in the root, you would run:

```sh
/ $ ls -la | grep -f .kind Dir
{name: etc, kind: Dir}
{name: mnt, kind: Dir}
{name: run, kind: Dir}
{name: var, kind: Dir}
{name: cdrom, kind: Dir}
{name: usr, kind: Dir}
{name: srv, kind: Dir}
{name: root, kind: Dir}
{name: media, kind: Dir}
{name: snap, kind: Dir}
{name: sys, kind: Dir}
{name: boot, kind: Dir}
{name: opt, kind: Dir}
{name: home, kind: Dir}
{name: dev, kind: Dir}
{name: proc, kind: Dir}
{name: tmp, kind: Dir}
{name: lost+found, kind: Dir}
```

Then, if you wanted a list of only the **names** of directories in the root, you would run:

```sh
/ $ ls -la | grep -f .kind Dir | get .name
etc
mnt
run
var
cdrom
usr
srv
root
media
snap
sys
boot
opt
home
dev
proc
tmp
lost+found
```

You could also save the file records you wanted into a file, and load them for use later:

```sh
/ $ ls -la | grep -f .kind Dir >files.cbor

/ $ get <files.cbor .name
etc
mnt
run
# ...etc.

/ $ get <files.cbor .name | sed etc 'something else here'
something else here
mnt
run
```

The shell can also catch common type errors, if it knows that you're attempting to pipe together two commands that expect different kinds of data:

```sh
/ $ ps | get
monch: type mismatch: cannot connect [unknown] (produced by ps) to cbor (expected by get)
```

It will also fail with an error if you perform a an I/O redirection that ignores data, like one in the middle of the pipeline:

```sh
/ $ echo test >file | cat
monch:  --> 1:11
  |
1 | echo test >file | cat
  |           ^---^
  |
  = cannot redirect output unless it's from the last command in a pipeline

/ $ echo test | cat <file
monch:  --> 1:12
  |
1 | cat | echo <file
  |            ^---^
  |
  = cannot redirect input outside unless it's from the first command in a pipeline
```

## Continuous Integration

For each commit, we run the test suite across Mac, Windows, and Linux in [GitHub Actions](https://github.com/wgoodall01/monch/actions).

We also make sure each commit successfully builds with `--release` and optimizations enabled.
