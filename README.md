# Cargo 3PL

:truck: The easy way to ship dependency licenses with your Rust binaries

[![Build Status](https://github.com/ankane/cargo-3pl/workflows/build/badge.svg?branch=master)](https://github.com/ankane/cargo-3pl/actions)

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

Specify specific target(s)

```sh
cargo 3pl --target x86_64-unknown-linux-gnu
```

## Missing License Files

If any packages are missing license files, create a new file:

```text

================================================================================
some-package LICENSE.txt
================================================================================

...

================================================================================
other-package COPYING
================================================================================

...
```

And append it:

```sh
cat LICENSE-MANUAL >> LICENSE-THIRD-PARTY
```

We also recommend creating a pull request for the package.

## History

View the [changelog](CHANGELOG.md)

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
