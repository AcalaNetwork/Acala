// This file is part of Acala.

// Copyright (C) 2022 Acala Foundation.
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

use super::Rate;

use codec::{Decode, Encode};
use frame_support::traits::Get;
use primitives::{Balance, BlockNumber};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_runtime::{
	traits::{CheckedSub, One, Zero},
	FixedPointNumber, RuntimeDebug,
};
use sp_std::{marker::PhantomData, prelude::*, result::Result};

pub enum Error {
	OutOfBound,
	ExceedMaxChangeAbs,
}

//TODO: manually implement Deserialize and Decode?
#[cfg_attr(feature = "std", derive(Serialize, Deserialize), serde(transparent))]
#[derive(Encode, Decode, PartialEq, Eq, PartialOrd, Ord, Copy, Clone, TypeInfo, RuntimeDebug)]
#[scale_info(skip_type_params(Range, MaxChangeAbs))]
pub struct BoundedType<T: Encode + Decode, Range, MaxChangeAbs>(
	T,
	#[cfg_attr(feature = "std", serde(skip_serializing))] PhantomData<(Range, MaxChangeAbs)>,
);

impl<T: Default + Encode + Decode, Range: Get<(T, T)>, MaxChangeAbs: Get<T>> Default
	for BoundedType<T, Range, MaxChangeAbs>
{
	fn default() -> Self {
		let (min, _) = Range::get();
		Self(min, PhantomData)
	}
}

impl<T: Encode + Decode + CheckedSub + PartialOrd + Copy, Range: Get<(T, T)>, MaxChangeAbs: Get<T>>
	BoundedType<T, Range, MaxChangeAbs>
{
	pub fn try_from(value: T) -> Result<Self, Error> {
		let (min, max) = Range::get();
		if value < min || value > max {
			return Err(Error::OutOfBound);
		}
		Ok(Self(value, PhantomData))
	}

	pub fn set(&mut self, value: T) -> Result<(), Error> {
		let (min, max) = Range::get();
		let max_change_abs = MaxChangeAbs::get();
		let old_value = &self.0;
		if value < min || value > max {
			return Err(Error::OutOfBound);
		}

		let abs = if value > *old_value {
			value
				.checked_sub(&old_value)
				.expect("greater number subtracting smaller one can't underflow; qed")
		} else {
			old_value
				.checked_sub(&value)
				.expect("greater number subtracting smaller one can't underflow; qed")
		};
		if abs > max_change_abs {
			return Err(Error::ExceedMaxChangeAbs);
		}

		self.0 = value;
		Ok(())
	}

	pub fn get(&self) -> T {
		self.0
	}
}

#[derive(Clone, Copy)]
pub struct Fractional;
impl Get<(Rate, Rate)> for Fractional {
	fn get() -> (Rate, Rate) {
		(Rate::zero(), Rate::one())
	}
}

#[derive(Clone, Copy)]
pub struct OneFifth;
impl Get<Rate> for OneFifth {
	fn get() -> Rate {
		Rate::saturating_from_rational(1, 5)
	}
}

pub type BoundedTypeRate<Range, MaxChangeAbs> = BoundedType<Rate, Range, MaxChangeAbs>;

pub type FractionalRate = BoundedTypeRate<Fractional, OneFifth>;

pub type BoundedTypeBalance<Range, MaxChangeAbs> = BoundedType<Balance, Range, MaxChangeAbs>;

pub type BoundedTypeBlockNumber<Range, MaxChangeAbs> = BoundedType<BlockNumber, Range, MaxChangeAbs>;
