use ethereum_types::H160;
use serde::Deserialize;
use sp_core::Bytes;
use sp_rpc::number::NumberOrHex;

/// Call request
#[derive(Debug, Default, PartialEq, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct CallRequest {
	/// From
	pub from: Option<H160>,
	/// To
	pub to: Option<H160>,
	/// Gas Limit
	pub gas_limit: Option<u32>,
	/// Storage Limit
	pub storage_limit: Option<u32>,
	/// Value
	pub value: Option<NumberOrHex>,
	/// Data
	pub data: Option<Bytes>,
}
