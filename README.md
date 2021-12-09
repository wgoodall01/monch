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

## Continuous Integration

For each commit, we run the test suite across Mac, Windows, and Linux in [GitHub Actions](https://github.com/wgoodall01/monch/actions).

We also make sure each commit successfully builds with `--release` and optimizations enabled.
