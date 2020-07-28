//! Acala CLI library.
//!
//! This package has two Cargo features:
//!
//! - `cli` (default): exposes functions that parse command-line options, then
//!   start and run the
//! node as a CLI application.
//!
//! - `browser`: exposes the content of the `browser` module, which consists of
//!   exported symbols
//! that are meant to be passed through the `wasm-bindgen` utility and called
//! from JavaScript. Despite its name the produced WASM can theoretically also
//! be used from NodeJS, although this hasn't been tested.

#![warn(missing_docs)]

#[macro_use]
mod service;
mod chain_spec;
mod cli;
mod command;
mod executor;
mod rpc;

fn main() -> sc_cli::Result<()> {
	command::run()
}
