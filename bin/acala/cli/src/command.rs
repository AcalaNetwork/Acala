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

// Disable the following lints
#![allow(clippy::borrowed_box)]

use crate::cli::{Cli, Subcommand};
use sc_cli::{Role, RuntimeVersion, SubstrateCli};
use sc_service::ChainType;
use service::{chain_spec, IdentifyVariant};

fn get_exec_name() -> Option<String> {
	std::env::current_exe()
		.ok()
		.and_then(|pb| pb.file_name().map(|s| s.to_os_string()))
		.and_then(|s| s.into_string().ok())
}

impl SubstrateCli for Cli {
	fn impl_name() -> String {
		"Acala Node".into()
	}

	fn impl_version() -> String {
		env!("SUBSTRATE_CLI_IMPL_VERSION").into()
	}

	fn description() -> String {
		env!("CARGO_PKG_DESCRIPTION").into()
	}

	fn author() -> String {
		"Acala Developers".into()
	}

	fn support_url() -> String {
		"https://github.com/AcalaNetwork/Acala/issues".into()
	}

	fn copyright_start_year() -> i32 {
		2019
	}

	fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
		let id = if id.is_empty() {
			let n = get_exec_name().unwrap_or_default();
			["acala", "karura", "mandala"]
				.iter()
				.cloned()
				.find(|&chain| n.starts_with(chain))
				.unwrap_or("acala")
		} else {
			id
		};

		Ok(match id {
			#[cfg(feature = "with-mandala-runtime")]
			"dev" => Box::new(chain_spec::mandala::development_testnet_config()?),
			#[cfg(feature = "with-mandala-runtime")]
			"local" => Box::new(chain_spec::mandala::local_testnet_config()?),
			#[cfg(feature = "with-mandala-runtime")]
			"mandala" => Box::new(chain_spec::mandala::mandala_testnet_config()?),
			#[cfg(feature = "with-mandala-runtime")]
			"mandala-latest" => Box::new(chain_spec::mandala::latest_mandala_testnet_config()?),
			#[cfg(feature = "with-karura-runtime")]
			"karura" => Box::new(chain_spec::karura::karura_config()?),
			#[cfg(feature = "with-karura-runtime")]
			"karura-latest" => Box::new(chain_spec::karura::latest_karura_config()?),
			#[cfg(feature = "with-acala-runtime")]
			"acala" => Box::new(chain_spec::acala::acala_config()?),
			#[cfg(feature = "with-acala-runtime")]
			"acala-latest" => Box::new(chain_spec::acala::latest_acala_config()?),
			path => {
				let path = std::path::PathBuf::from(path);

				let starts_with = |prefix: &str| {
					path.file_name()
						.map(|f| f.to_str().map(|s| s.starts_with(&prefix)))
						.flatten()
						.unwrap_or(false)
				};

				if starts_with("karura") {
					#[cfg(feature = "with-karura-runtime")]
					{
						Box::new(chain_spec::karura::ChainSpec::from_json_file(path)?)
					}

					#[cfg(not(feature = "with-karura-runtime"))]
					return Err("Karura runtime is not available. Please compile the node with `--features with-karura-runtime` to enable it.".into());
				} else if starts_with("acala") {
					#[cfg(feature = "with-acala-runtime")]
					{
						Box::new(chain_spec::acala::ChainSpec::from_json_file(path)?)
					}
					#[cfg(not(feature = "with-acala-runtime"))]
					return Err("Acala runtime is not available. Please compile the node with `--features with-acala-runtime` to enable it.".into());
				} else {
					#[cfg(feature = "with-mandala-runtime")]
					{
						Box::new(chain_spec::mandala::ChainSpec::from_json_file(path)?)
					}
					#[cfg(not(feature = "with-mandala-runtime"))]
					return Err("Mandala runtime is not available. Please compile the node with `--features with-mandala-runtime` to enable it.".into());
				}
			}
		})
	}

	fn native_runtime_version(spec: &Box<dyn sc_service::ChainSpec>) -> &'static RuntimeVersion {
		if spec.is_acala() {
			#[cfg(feature = "with-acala-runtime")]
			return &service::acala_runtime::VERSION;
			#[cfg(not(feature = "with-acala-runtime"))]
			panic!("Acala runtime is not available. Please compile the node with `--features with-acala-runtime` to enable it.");
		} else if spec.is_karura() {
			#[cfg(feature = "with-karura-runtime")]
			return &service::karura_runtime::VERSION;
			#[cfg(not(feature = "with-karura-runtime"))]
			panic!("Karura runtime is not available. Please compile the node with `--features with-karura-runtime` to enable it.");
		} else {
			#[cfg(feature = "with-mandala-runtime")]
			return &service::mandala_runtime::VERSION;
			#[cfg(not(feature = "with-mandala-runtime"))]
			panic!("Mandala runtime is not available. Please compile the node with `--features with-mandala-runtime` to enable it.");
		}
	}
}

fn set_default_ss58_version(spec: &Box<dyn service::ChainSpec>) {
	use sp_core::crypto::Ss58AddressFormat;

	let ss58_version = if spec.is_karura() {
		Ss58AddressFormat::KaruraAccount
	} else if spec.is_acala() {
		Ss58AddressFormat::AcalaAccount
	} else {
		Ss58AddressFormat::SubstrateAccount
	};

	sp_core::crypto::set_default_ss58_version(ss58_version);
}

/// Parses acala specific CLI arguments and run the service.
pub fn run() -> sc_cli::Result<()> {
	let cli = Cli::from_args();

	match &cli.subcommand {
		None => {
			let runner = cli.create_runner(&cli.run)?;
			let chain_spec = &runner.config().chain_spec;

			set_default_ss58_version(chain_spec);

			if cli.instant_sealing && chain_spec.chain_type() != ChainType::Development {
				return Err("Instant sealing can be turned on only in `--dev` mode".into());
			}

			runner.run_node_until_exit(|config| async move {
				match config.role {
					Role::Light => service::build_light(config),
					_ => {
						service::build_full(config, cli.instant_sealing, false).map(|(_, _, task_manager)| task_manager)
					}
				}
				.map_err(sc_cli::Error::Service)
			})
		}

		Some(Subcommand::Inspect(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			let chain_spec = &runner.config().chain_spec;

			set_default_ss58_version(chain_spec);

			runner.sync_run(|config| {
				let (client, _, _) = service::build_full(config, false, false)?;
				cmd.run(client)
			})
		}

		Some(Subcommand::Benchmark(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			let chain_spec = &runner.config().chain_spec;

			set_default_ss58_version(chain_spec);

			#[cfg(feature = "with-acala-runtime")]
			return runner.sync_run(|config| cmd.run::<service::acala_runtime::Block, service::AcalaExecutor>(config));

			#[cfg(feature = "with-karura-runtime")]
			return runner
				.sync_run(|config| cmd.run::<service::karura_runtime::Block, service::KaruraExecutor>(config));

			#[cfg(feature = "with-mandala-runtime")]
			return runner
				.sync_run(|config| cmd.run::<service::mandala_runtime::Block, service::MandalaExecutor>(config));
		}

		Some(Subcommand::Key(cmd)) => cmd.run(&cli),
		Some(Subcommand::Sign(cmd)) => cmd.run(),
		Some(Subcommand::Verify(cmd)) => cmd.run(),
		Some(Subcommand::Vanity(cmd)) => cmd.run(),

		Some(Subcommand::BuildSpec(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
		}

		Some(Subcommand::CheckBlock(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			let chain_spec = &runner.config().chain_spec;

			set_default_ss58_version(chain_spec);

			runner.async_run(|mut config| {
				let (client, _, import_queue, task_manager) = service::new_chain_ops(&mut config)?;
				Ok((cmd.run(client, import_queue), task_manager))
			})
		}

		Some(Subcommand::ExportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			let chain_spec = &runner.config().chain_spec;

			set_default_ss58_version(chain_spec);

			runner.async_run(|mut config| {
				let (client, _, _, task_manager) = service::new_chain_ops(&mut config)?;
				Ok((cmd.run(client, config.database), task_manager))
			})
		}

		Some(Subcommand::ExportState(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			let chain_spec = &runner.config().chain_spec;

			set_default_ss58_version(chain_spec);

			runner.async_run(|mut config| {
				let (client, _, _, task_manager) = service::new_chain_ops(&mut config)?;
				Ok((cmd.run(client, config.chain_spec), task_manager))
			})
		}

		Some(Subcommand::ImportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			let chain_spec = &runner.config().chain_spec;

			set_default_ss58_version(chain_spec);

			runner.async_run(|mut config| {
				let (client, _, import_queue, task_manager) = service::new_chain_ops(&mut config)?;
				Ok((cmd.run(client, import_queue), task_manager))
			})
		}

		Some(Subcommand::PurgeChain(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run(config.database))
		}

		Some(Subcommand::Revert(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			let chain_spec = &runner.config().chain_spec;

			set_default_ss58_version(chain_spec);

			runner.async_run(|mut config| {
				let (client, backend, _, task_manager) = service::new_chain_ops(&mut config)?;
				Ok((cmd.run(client, backend), task_manager))
			})
		}
	}
}
