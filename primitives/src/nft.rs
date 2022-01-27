// This file is part of Acala.

// Copyright (C) 2020-2022 Acala Foundation.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use codec::{Decode, Encode};
use scale_info::{build::Fields, meta_type, Path, Type, TypeInfo, TypeParameter};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use frame_support::RuntimeDebug;
use sp_std::{collections::btree_map::BTreeMap, prelude::*};

use enumflags2::BitFlags;

pub type NFTBalance = u128;
pub type CID = Vec<u8>;
pub type Attributes = BTreeMap<Vec<u8>, Vec<u8>>;

#[repr(u8)]
#[derive(Encode, Decode, Clone, Copy, BitFlags, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub enum ClassProperty {
	/// Is token transferable
	Transferable = 0b00000001,
	/// Is token burnable
	Burnable = 0b00000010,
	/// Is minting new tokens allowed
	Mintable = 0b00000100,
	/// Is class properties mutable
	ClassPropertiesMutable = 0b00001000,
}

#[derive(Clone, Copy, PartialEq, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Properties(pub BitFlags<ClassProperty>);

impl Eq for Properties {}
impl Encode for Properties {
	fn using_encoded<R, F: FnOnce(&[u8]) -> R>(&self, f: F) -> R {
		self.0.bits().using_encoded(f)
	}
}
impl Decode for Properties {
	fn decode<I: codec::Input>(input: &mut I) -> sp_std::result::Result<Self, codec::Error> {
		let field = u8::decode(input)?;
		Ok(Self(
			<BitFlags<ClassProperty>>::from_bits(field as u8).map_err(|_| "invalid value")?,
		))
	}
}

impl TypeInfo for Properties {
	type Identity = Self;

	fn type_info() -> Type {
		Type::builder()
			.path(Path::new("BitFlags", module_path!()))
			.type_params(vec![TypeParameter::new("T", Some(meta_type::<ClassProperty>()))])
			.composite(Fields::unnamed().field(|f| f.ty::<u8>().type_name("ClassProperty")))
	}
}
