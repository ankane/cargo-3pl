# Cargo 3PL

🚚 The easy way to ship dependency licenses with your Rust binaries

[![Build Status](https://github.com/ankane/cargo-3pl/actions/workflows/build.yml/badge.svg)](https://github.com/ankane/cargo-3pl/actions)

## Installation

Run:

```sh
cargo install cargo-3pl
```

## Getting Started

As part of your distribution process, run:

```sh
cargo 3pl > LICENSE-THIRD-PARTY
```

## How It Works

This project creates a summary of your dependency licenses from their `license` field in `Cargo.toml`. It then tries to find their license files. It looks for:

- filenames that contain `LICENSE`, `LICENCE`, `NOTICE`, or `COPYING` (case-insensitive)
- `txt`, `md`, or no extension (case-insensitive)

Dependencies in the current workspace are not included.

## Options

Specify features to include

```sh
cargo 3pl --features <FEATURES>...
cargo 3pl --all-features
cargo 3pl --no-default-features
```

Specify targets

```sh
cargo 3pl --target x86_64-unknown-linux-gnu
```

## Missing License Files

If any crates are missing license files, create a directory for licenses:

```sh
mkdir -p 3pl-source/some-crate-0.1.0
cp /path/to/some-crate/LICENSE 3pl-source/some-crate-0.1.0
```

And use:

```sh
cargo 3pl --source 3pl-source
```

See an [example](https://github.com/ankane/3pl-source)

## History

View the [changelog](https://github.com/ankane/cargo-3pl/blob/master/CHANGELOG.md)

## Contributing

Everyone is encouraged to help improve this project. Here are a few ways you can help:

- [Report bugs](https://github.com/ankane/cargo-3pl/issues)
- Fix bugs and [submit pull requests](https://github.com/ankane/cargo-3pl/pulls)
- Write, clarify, or fix documentation
- Suggest or add new features

To get started with development:

```sh
git clone https://github.com/ankane/cargo-3pl.git
cd cargo-3pl
cargo run
```
