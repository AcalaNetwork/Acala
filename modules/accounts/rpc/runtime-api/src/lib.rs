//! Runtime API definition for accounts module.

#![cfg_attr(not(feature = "std"), no_std)]
// The `too_many_arguments` warning originates from `decl_runtime_apis` macro.
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_mut_passed)]

use codec::{Codec, Decode, Encode};
use frame_support::weights::{DispatchClass, Weight};
#[cfg(feature = "std")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sp_runtime::traits::{MaybeDisplay, MaybeFromStr};
use sp_std::prelude::*;

/// Information related to a dispatchable's class, weight, and fee that can be
/// queried from the runtime.
#[derive(Eq, PartialEq, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct RuntimeDispatchInfo<Balance> {
	/// Weight of this dispatch.
	pub weight: Weight,
	/// Class of this dispatch.
	pub class: DispatchClass,
	/// The inclusion fee of this dispatch. This does not include a tip or
	/// anything else that depends on the signature (i.e. depends on a
	/// `SignedExtension`).
	#[cfg_attr(feature = "std", serde(bound(serialize = "Balance: std::fmt::Display")))]
	#[cfg_attr(feature = "std", serde(serialize_with = "serialize_as_string"))]
	#[cfg_attr(feature = "std", serde(bound(deserialize = "Balance: std::str::FromStr")))]
	#[cfg_attr(feature = "std", serde(deserialize_with = "deserialize_from_string"))]
	pub partial_fee: Balance,
}

#[cfg(feature = "std")]
fn serialize_as_string<S: Serializer, T: std::fmt::Display>(t: &T, serializer: S) -> Result<S::Ok, S::Error> {
	serializer.serialize_str(&t.to_string())
}

#[cfg(feature = "std")]
fn deserialize_from_string<'de, D: Deserializer<'de>, T: std::str::FromStr>(deserializer: D) -> Result<T, D::Error> {
	let s = String::deserialize(deserializer)?;
	s.parse::<T>()
		.map_err(|_| serde::de::Error::custom("Parse from string failed"))
}

sp_api::decl_runtime_apis! {
	pub trait TransactionPaymentApi<Balance> where
		Balance: Codec + MaybeDisplay + MaybeFromStr,
	{
		fn query_info(uxt: Block::Extrinsic, len: u32) -> RuntimeDispatchInfo<Balance>;
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn should_serialize_and_deserialize_properly_with_string() {
		let info = RuntimeDispatchInfo {
			weight: 5,
			class: DispatchClass::Normal,
			partial_fee: 1_000_000_u64,
		};

		let json_str = r#"{"weight":5,"class":"normal","partialFee":"1000000"}"#;

		assert_eq!(serde_json::to_string(&info).unwrap(), json_str);
		assert_eq!(
			serde_json::from_str::<RuntimeDispatchInfo<u64>>(json_str).unwrap(),
			info
		);

		// should not panic
		serde_json::to_value(&info).unwrap();
	}

	#[test]
	fn should_serialize_and_deserialize_properly_large_value() {
		let info = RuntimeDispatchInfo {
			weight: 5,
			class: DispatchClass::Normal,
			partial_fee: u128::max_value(),
		};

		let json_str = r#"{"weight":5,"class":"normal","partialFee":"340282366920938463463374607431768211455"}"#;

		assert_eq!(serde_json::to_string(&info).unwrap(), json_str);
		assert_eq!(
			serde_json::from_str::<RuntimeDispatchInfo<u128>>(json_str).unwrap(),
			info
		);

		// should not panic
		serde_json::to_value(&info).unwrap();
	}
}
