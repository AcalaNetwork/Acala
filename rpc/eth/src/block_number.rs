// Copyright 2015-2020 Parity Technologies (UK) Ltd.
// This file is part of Frontier.

// Open Ethereum is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Open Ethereum is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Open Ethereum.  If not, see <http://www.gnu.org/licenses/>.

use ethereum_types::H256;
use serde::de::{Error, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// Represents rpc api block number param.
#[derive(Debug, PartialEq, Clone, Hash, Eq)]
pub enum BlockNumber {
	/// Hash
	Hash {
		/// block hash
		hash: H256,
		/// only return blocks part of the canon chain
		require_canonical: bool,
	},
	/// Number
	Num(u64),
	/// Latest block
	Latest,
	/// Earliest block (genesis)
	Earliest,
	/// Pending block (being mined)
	Pending,
}

impl Default for BlockNumber {
	fn default() -> Self {
		BlockNumber::Latest
	}
}

impl<'a> Deserialize<'a> for BlockNumber {
	fn deserialize<D>(deserializer: D) -> Result<BlockNumber, D::Error>
	where
		D: Deserializer<'a>,
	{
		deserializer.deserialize_any(BlockNumberVisitor)
	}
}

impl BlockNumber {
	/// Convert block number to min block target.
	pub fn to_min_block_num(&self) -> Option<u64> {
		match *self {
			BlockNumber::Num(ref x) => Some(*x),
			_ => None,
		}
	}
}

impl Serialize for BlockNumber {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		match *self {
			BlockNumber::Hash {
				hash,
				require_canonical,
			} => serializer.serialize_str(&format!(
				"{{ 'hash': '{}', 'requireCanonical': '{}'  }}",
				hash, require_canonical
			)),
			BlockNumber::Num(ref x) => serializer.serialize_str(&format!("0x{:x}", x)),
			BlockNumber::Latest => serializer.serialize_str("latest"),
			BlockNumber::Earliest => serializer.serialize_str("earliest"),
			BlockNumber::Pending => serializer.serialize_str("pending"),
		}
	}
}

struct BlockNumberVisitor;

impl<'a> Visitor<'a> for BlockNumberVisitor {
	type Value = BlockNumber;

	fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		write!(formatter, "a block number or 'latest', 'earliest' or 'pending'")
	}

	fn visit_map<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
	where
		V: MapAccess<'a>,
	{
		let (mut require_canonical, mut block_number, mut block_hash) = (false, None::<u64>, None::<H256>);

		loop {
			let key_str: Option<String> = visitor.next_key()?;

			match key_str {
				Some(key) => match key.as_str() {
					"blockNumber" => {
						let value: String = visitor.next_value()?;
						if value.starts_with("0x") {
							let number = u64::from_str_radix(&value[2..], 16)
								.map_err(|e| Error::custom(format!("Invalid block number: {}", e)))?;

							block_number = Some(number);
							break;
						} else {
							return Err(Error::custom("Invalid block number: missing 0x prefix".to_string()));
						}
					}
					"blockHash" => {
						block_hash = Some(visitor.next_value()?);
					}
					"requireCanonical" => {
						require_canonical = visitor.next_value()?;
					}
					key => return Err(Error::custom(format!("Unknown key: {}", key))),
				},
				None => break,
			};
		}

		if let Some(number) = block_number {
			return Ok(BlockNumber::Num(number));
		}

		if let Some(hash) = block_hash {
			return Ok(BlockNumber::Hash {
				hash,
				require_canonical,
			});
		}

		return Err(Error::custom("Invalid input"));
	}

	fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
	where
		E: Error,
	{
		match value {
			"latest" => Ok(BlockNumber::Latest),
			"earliest" => Ok(BlockNumber::Earliest),
			"pending" => Ok(BlockNumber::Pending),
			_ if value.starts_with("0x") => u64::from_str_radix(&value[2..], 16)
				.map(BlockNumber::Num)
				.map_err(|e| Error::custom(format!("Invalid block number: {}", e))),
			_ => Err(Error::custom("Invalid block number: missing 0x prefix".to_string())),
		}
	}

	fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
	where
		E: Error,
	{
		self.visit_str(value.as_ref())
	}
}
