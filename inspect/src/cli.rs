use sc_cli::{ImportParams, SharedParams};
use std::fmt::Debug;
use structopt::StructOpt;

/// The `inspect` command used to print decoded chain data.
#[derive(Debug, StructOpt)]
pub struct InspectCmd {
	#[allow(missing_docs)]
	#[structopt(flatten)]
	pub command: InspectSubCmd,

	#[allow(missing_docs)]
	#[structopt(flatten)]
	pub shared_params: SharedParams,

	#[allow(missing_docs)]
	#[structopt(flatten)]
	pub import_params: ImportParams,
}

/// A possible inspect sub-commands.
#[derive(Debug, StructOpt)]
pub enum InspectSubCmd {
	/// Decode block with native version of runtime and print out the details.
	Block {
		/// Address of the block to print out.
		///
		/// Can be either a block hash (no 0x prefix) or a number to retrieve
		/// existing block, or a 0x-prefixed bytes hex string, representing
		/// SCALE encoding of a block.
		#[structopt(value_name = "HASH or NUMBER or BYTES")]
		input: String,
	},
	/// Decode extrinsic with native version of runtime and print out the
	/// details.
	Extrinsic {
		/// Address of an extrinsic to print out.
		///
		/// Can be either a block hash (no 0x prefix) or number and the index,
		/// in the form of `{block}:{index}` or a 0x-prefixed bytes hex string,
		/// representing SCALE encoding of an extrinsic.
		#[structopt(value_name = "BLOCK:INDEX or BYTES")]
		input: String,
	},
}
