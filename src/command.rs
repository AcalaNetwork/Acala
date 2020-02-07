use crate::chain_spec;
use crate::cli::Cli;
use crate::service;
use sc_cli::{error, VersionInfo};

/// Parse and run command line arguments
pub fn run(version: VersionInfo) -> error::Result<()> {
	let opt = sc_cli::from_args::<Cli>(&version);

	let config = sc_service::Configuration::new(&version);

	match opt.subcommand {
		Some(subcommand) => sc_cli::run_subcommand(
			config,
			subcommand,
			chain_spec::load_spec,
			|config: _| Ok(new_full_start!(config).0),
			&version,
		),
		None => sc_cli::run(
			config,
			opt.run,
			service::new_light,
			service::new_full,
			chain_spec::load_spec,
			&version,
		),
	}
}
