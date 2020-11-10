//! Acala CLI library.

use sc_cli::{KeySubcommand, RunCmd, SignCmd, VanityCmd, VerifyCmd};
use structopt::StructOpt;

/// Possible subcommands of the main binary.
#[derive(Debug, StructOpt)]
pub enum Subcommand {
	/// Key management cli utilities
	Key(KeySubcommand),

	/// The custom inspect subcommmand for decoding blocks and extrinsics.
	#[structopt(
		name = "inspect",
		about = "Decode given block or extrinsic using current native runtime."
	)]
	Inspect(inspect::cli::InspectCmd),

	/// The custom benchmark subcommmand benchmarking runtime modules.
	#[structopt(name = "benchmark", about = "Benchmark runtime modules.")]
	Benchmark(frame_benchmarking_cli::BenchmarkCmd),

	/// Verify a signature for a message, provided on STDIN, with a given
	/// (public or secret) key.
	Verify(VerifyCmd),

	/// Generate a seed that provides a vanity address.
	Vanity(VanityCmd),

	/// Sign a message, with a given (secret) key.
	Sign(SignCmd),

	/// Build a chain specification.
	BuildSpec(sc_cli::BuildSpecCmd),

	/// Validate blocks.
	CheckBlock(sc_cli::CheckBlockCmd),

	/// Export blocks.
	ExportBlocks(sc_cli::ExportBlocksCmd),

	/// Export the state of a given block into a chain spec.
	ExportState(sc_cli::ExportStateCmd),

	/// Import blocks.
	ImportBlocks(sc_cli::ImportBlocksCmd),

	/// Remove the whole chain.
	PurgeChain(sc_cli::PurgeChainCmd),

	/// Revert the chain to a previous state.
	Revert(sc_cli::RevertCmd),
}

/// An overarching CLI command definition.
#[derive(Debug, StructOpt)]
pub struct Cli {
	/// Possible subcommand with parameters.
	#[structopt(subcommand)]
	pub subcommand: Option<Subcommand>,

	#[allow(missing_docs)]
	#[structopt(flatten)]
	pub run: RunCmd,
}
