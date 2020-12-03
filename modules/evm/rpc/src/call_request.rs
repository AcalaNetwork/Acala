use ethereum_types::H160;
use serde::{Deserialize, Deserializer};
use sp_core::Bytes;

/// Call request
#[derive(Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct CallRequest<Balance> {
	/// From
	pub from: Option<H160>,
	/// To
	pub to: Option<H160>,
	/// Gas Limit
	pub gas_limit: Option<u32>,
	/// Value
	#[serde(default)]
	#[serde(bound(deserialize = "Balance: std::str::FromStr"))]
	#[serde(deserialize_with = "deserialize_from_string")]
	pub value: Option<Balance>,
	/// Data
	pub data: Option<Bytes>,
}

fn deserialize_from_string<'de, D: Deserializer<'de>, T: std::str::FromStr>(
	deserializer: D,
) -> Result<Option<T>, D::Error> {
	let s = Option::<String>::deserialize(deserializer)?;
	if let Some(s) = s {
		s.parse::<T>()
			.map(Some)
			.map_err(|_| serde::de::Error::custom("Parse from string failed"))
	} else {
		Ok(None)
	}
}
