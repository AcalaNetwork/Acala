// Disable the following lints
#![allow(clippy::borrowed_box)]

use crate::cli::{Cli, Subcommand};
use sc_cli::{Role, RuntimeVersion, SubstrateCli};
use service::IdentifyVariant;

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
		Ok(match id {
			"dev" => Box::new(service::chain_spec::development_testnet_config()?),
			"local" => Box::new(service::chain_spec::local_testnet_config()?),
			"" | "mandala" => Box::new(service::chain_spec::mandala_testnet_config()?),
			"mandala-latest" => Box::new(service::chain_spec::latest_mandala_testnet_config()?),
			path => Box::new(service::chain_spec::DevChainSpec::from_json_file(
				std::path::PathBuf::from(path),
			)?),
		})
	}

	fn native_runtime_version(_: &Box<dyn sc_service::ChainSpec>) -> &'static RuntimeVersion {
		&service::dev_runtime::VERSION
	}
}

/// Parses acala specific CLI arguments and run the service.
pub fn run() -> sc_cli::Result<()> {
	let cli = Cli::from_args();

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
	};

	match &cli.subcommand {
		None => {
			let runner = cli.create_runner(&cli.run)?;
			let chain_spec = &runner.config().chain_spec;

			set_default_ss58_version(chain_spec);

			runner.run_node_until_exit(|config| match config.role {
				Role::Light => {
					service::new_light::<service::dev_runtime::RuntimeApi, service::DevExecutor>(config).map(|r| r.0)
				}
				_ => service::new_full::<service::dev_runtime::RuntimeApi, service::DevExecutor, _>(
					config,
					|_, _| (),
					false,
				)
				.map(|r| r.0),
			})
		}

		Some(Subcommand::Inspect(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			let chain_spec = &runner.config().chain_spec;

			set_default_ss58_version(chain_spec);

			runner.sync_run(|config| {
				cmd.run::<service::dev_runtime::Block, service::dev_runtime::RuntimeApi, service::DevExecutor>(config)
			})
		}

		Some(Subcommand::Benchmark(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			let chain_spec = &runner.config().chain_spec;

			set_default_ss58_version(chain_spec);

			runner.sync_run(|config| cmd.run::<service::dev_runtime::Block, service::DevExecutor>(config))
		}

		Some(Subcommand::Key(cmd)) => cmd.run(),
		Some(Subcommand::Sign(cmd)) => cmd.run(),
		Some(Subcommand::Verify(cmd)) => cmd.run(),
		Some(Subcommand::Vanity(cmd)) => cmd.run(),

		Some(Subcommand::BuildSpec(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
		}

		Some(Subcommand::BuildSyncSpec(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			let chain_spec = &runner.config().chain_spec;

			set_default_ss58_version(chain_spec);

			runner.async_run(|config| {
				let chain_spec = config.chain_spec.cloned_box();
				let network_config = config.network.clone();
				let (task_manager, _, client, _, _, network_status_sinks) = service::new_full::<
					service::dev_runtime::RuntimeApi,
					service::DevExecutor,
					_,
				>(config, |_, _| (), false)?;

				Ok((
					cmd.run(chain_spec, network_config, client, network_status_sinks),
					task_manager,
				))
			})
		}

		Some(Subcommand::CheckBlock(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			let chain_spec = &runner.config().chain_spec;

			set_default_ss58_version(chain_spec);

			runner.async_run(|mut config| {
				let (client, _, import_queue, task_manager) =
					service::new_chain_ops::<service::dev_runtime::RuntimeApi, service::DevExecutor>(&mut config)?;
				Ok((cmd.run(client, import_queue), task_manager))
			})
		}

		Some(Subcommand::ExportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			let chain_spec = &runner.config().chain_spec;

			set_default_ss58_version(chain_spec);

			runner.async_run(|mut config| {
				let (client, _, _, task_manager) =
					service::new_chain_ops::<service::dev_runtime::RuntimeApi, service::DevExecutor>(&mut config)?;
				Ok((cmd.run(client, config.database), task_manager))
			})
		}

		Some(Subcommand::ExportState(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			let chain_spec = &runner.config().chain_spec;

			set_default_ss58_version(chain_spec);

			runner.async_run(|mut config| {
				let (client, _, _, task_manager) =
					service::new_chain_ops::<service::dev_runtime::RuntimeApi, service::DevExecutor>(&mut config)?;
				Ok((cmd.run(client, config.chain_spec), task_manager))
			})
		}

		Some(Subcommand::ImportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			let chain_spec = &runner.config().chain_spec;

			set_default_ss58_version(chain_spec);

			runner.async_run(|mut config| {
				let (client, _, import_queue, task_manager) =
					service::new_chain_ops::<service::dev_runtime::RuntimeApi, service::DevExecutor>(&mut config)?;
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
				let (client, backend, _, task_manager) =
					service::new_chain_ops::<service::dev_runtime::RuntimeApi, service::DevExecutor>(&mut config)?;
				Ok((cmd.run(client, backend), task_manager))
			})
		}
	}
}
