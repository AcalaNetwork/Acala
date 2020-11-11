use ethereum_types::{H160, U256};
use serde::Deserialize;
use sp_core::Bytes;

/// Call request
#[derive(Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct CallRequest {
	/// From
	pub from: Option<H160>,
	/// To
	pub to: Option<H160>,
	/// Gas Limit
	pub gas_limit: Option<u32>,
	/// Value
	pub value: Option<U256>,
	/// Data
	pub data: Option<Bytes>,
}
