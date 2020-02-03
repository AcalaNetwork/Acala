use crate::chain_spec;
use crate::cli::Cli;
use crate::new_full_start;
use crate::service;
use sc_cli::{error, VersionInfo};
use sp_consensus_aura::sr25519::AuthorityPair as AuraPair;

/// Parse and run command line arguments
pub fn run(version: VersionInfo) -> error::Result<()> {
	let opt = sc_cli::from_args::<Cli>(&version);

	let mut config = sc_service::Configuration::default();
	config.impl_name = "acala";

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
