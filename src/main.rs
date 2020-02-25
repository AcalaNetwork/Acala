mod chain_spec;
mod rpc;
#[macro_use]
mod service;
mod cli;
mod command;
mod executor;

fn main() -> sc_cli::Result<()> {
	let version = sc_cli::VersionInfo {
		name: "Acala",
		commit: env!("VERGEN_SHA_SHORT"),
		version: env!("CARGO_PKG_VERSION"),
		executable_name: "acala",
		author: "Acala Developers",
		description: "acala",
		support_url: "https://github.com/AcalaNetwork/Acala/issues",
		copyright_start_year: 2020,
	};

	command::run(version)
}
