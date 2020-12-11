//! Acala CLI library.
//!
//! This package has two Cargo features:
//!
//! - `cli` (default): exposes functions that parse command-line options, then
//!   start and run the
//! node as a CLI application.
//!
//! TODO :
//! - `browser`: exposes the content of the `browser` module, which consists of
//!   exported symbols
//! that are meant to be passed through the `wasm-bindgen` utility and called
//! from JavaScript. Despite its name the produced WASM can theoretically also
//! be used from NodeJS, although this hasn't been tested.

#![warn(missing_docs)]

fn main() -> acala_cli::Result<()> {
	acala_cli::run()
}
