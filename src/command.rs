use crate::executor::Executor;
use crate::{
	chain_spec,
	cli::{Cli, Subcommand},
	service,
};
use runtime::{Block, RuntimeApi};
use sc_cli::{Result, SubstrateCli};
use sc_finality_grandpa as grandpa;

impl SubstrateCli for Cli {
	fn impl_name() -> &'static str {
		"Acala Node"
	}

	fn impl_version() -> &'static str {
		env!("CARGO_PKG_VERSION")
	}

	fn description() -> &'static str {
		env!("CARGO_PKG_DESCRIPTION")
	}

	fn author() -> &'static str {
		"Acala Developers"
	}

	fn support_url() -> &'static str {
		"https://github.com/AcalaNetwork/Acala/issues"
	}

	fn copyright_start_year() -> i32 {
		2020
	}

	fn executable_name() -> &'static str {
		"acala"
	}

	fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
		Ok(match id {
			"dev" => Box::new(chain_spec::development_testnet_config()),
			"local" => Box::new(chain_spec::local_testnet_config()),
			"" | "mandala" => Box::new(chain_spec::mandala_testnet_config()?),
			"mandala-latest" => Box::new(chain_spec::latest_mandala_testnet_config()),
			path => Box::new(chain_spec::ChainSpec::from_json_file(std::path::PathBuf::from(path))?),
		})
	}
}

/// Parse command line arguments into service configuration.
pub fn run() -> Result<()> {
	let cli = Cli::from_args();

	match &cli.subcommand {
		None => {
			let runner = cli.create_runner(&cli.run)?;
			runner.run_node(service::new_light, service::new_full, runtime::VERSION)
		}

		Some(Subcommand::Base(subcommand)) => {
			let runner = cli.create_runner(subcommand)?;

			runner.run_subcommand(subcommand, |config| Ok(new_full_start!(config).0))
		}

		Some(Subcommand::Inspect(cmd)) => {
			let runner = cli.create_runner(cmd)?;

			runner.sync_run(|config| cmd.run::<Block, RuntimeApi, Executor>(config))
		}

		Some(Subcommand::Benchmark(cmd)) => {
			if cfg!(feature = "runtime-benchmarks") {
				let runner = cli.create_runner(cmd)?;

				runner.sync_run(|config| cmd.run::<Block, Executor>(config))
			} else {
				println!(
					"Benchmarking wasn't enabled when building the node. \
				You can enable it with `--features runtime-benchmarks`."
				);
				Ok(())
			}
		}
	}
}
