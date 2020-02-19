mod chain_spec;
mod rpc;
#[macro_use]
mod service;
mod cli;
mod command;
mod executor;

pub use sc_cli::{error, VersionInfo};

fn main() -> Result<(), error::Error> {
	let version = VersionInfo {
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
