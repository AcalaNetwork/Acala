// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Acala dev CLI library.
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

// #![warn(missing_docs)]

fn main() -> acala_dev_cli::Result<()> {
	acala_dev_cli::run()
}
