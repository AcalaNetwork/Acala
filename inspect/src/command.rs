// This file is part of Acala.

// Copyright (C) 2020-2022 Acala Foundation.
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

//! Command ran by the CLI

use crate::cli::{InspectCmd, InspectSubCmd};
use crate::Inspector;
use sc_cli::{CliConfiguration, ImportParams, Result, SharedParams};
use sc_client_api::BlockBackend;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block;
use std::str::FromStr;
use std::sync::Arc;

impl InspectCmd {
	/// Run the inspect command, passing the inspector.
	pub fn run<B, CL>(&self, client: Arc<CL>) -> Result<()>
	where
		B: Block,
		B::Hash: FromStr,
		CL: BlockBackend<B> + HeaderBackend<B> + 'static,
	{
		match Arc::try_unwrap(client) {
			Ok(cli) => {
				let inspect = Inspector::<B>::new(cli);

				match &self.command {
					InspectSubCmd::Block { input } => {
						let input = input.parse()?;
						let res = inspect.block(input).map_err(|e| format!("{}", e))?;
						println!("{}", res);
						Ok(())
					}
					InspectSubCmd::Extrinsic { input } => {
						let input = input.parse()?;
						let res = inspect.extrinsic(input).map_err(|e| format!("{}", e))?;
						println!("{}", res);
						Ok(())
					}
				}
			}

			Err(_) => Err("Client try_unwrap failed".into()),
		}
	}
}

impl CliConfiguration for InspectCmd {
	fn shared_params(&self) -> &SharedParams {
		&self.shared_params
	}

	fn import_params(&self) -> Option<&ImportParams> {
		Some(&self.import_params)
	}
}
