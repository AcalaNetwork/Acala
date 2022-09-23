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

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::traits::Get;
use primitives::{Balance, BlockNumber};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{CheckedSub, One, Zero},
	FixedPointNumber, RuntimeDebug,
};
use sp_std::{marker::PhantomData, prelude::*, result::Result};

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

/// The bounded type errors.
#[derive(RuntimeDebug, PartialEq, Eq)]
pub enum Error {
	/// The value is out of bound.
	OutOfBound,
	/// The change diff exceeds the max absolute value.
	ExceedMaxChangeAbs,
}

/// An abstract definition of bounded type. The type is within the range of `Range`
/// and while update the inner value, the max absolute value of the diff is `MaxChangeAbs`.
/// The `Default` value is minimum value of the range.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize), serde(transparent))]
#[derive(Encode, Decode, PartialEq, Eq, PartialOrd, Ord, Copy, Clone, TypeInfo, MaxEncodedLen, RuntimeDebug)]
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
	/// Try to create a new instance of `BoundedType`. Returns `Err` if out of bound.
	pub fn try_from(value: T) -> Result<Self, Error> {
		let (min, max) = Range::get();
		if value < min || value > max {
			return Err(Error::OutOfBound);
		}
		Ok(Self(value, PhantomData))
	}

	/// Set the inner value. Returns `Err` if out of bound or the diff with current value exceeds
	/// the max absolute value.
	pub fn set(&mut self, value: T) -> Result<(), Error> {
		let (min, max) = Range::get();
		let max_change_abs = MaxChangeAbs::get();
		let old_value = &self.0;
		if value < min || value > max {
			return Err(Error::OutOfBound);
		}

		let abs = if value > *old_value {
			value
				.checked_sub(old_value)
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

/// Fractional range between `Rate::zero()` and `Rate::one()`.
#[derive(Clone, Copy, PartialEq, Eq, RuntimeDebug)]
pub struct Fractional;
impl Get<(Rate, Rate)> for Fractional {
	fn get() -> (Rate, Rate) {
		(Rate::zero(), Rate::one())
	}
}

/// Maximum absolute change is 1/5.
#[derive(Clone, Copy, PartialEq, Eq, RuntimeDebug)]
pub struct OneFifth;
impl Get<Rate> for OneFifth {
	fn get() -> Rate {
		Rate::saturating_from_rational(1, 5)
	}
}

pub type BoundedRate<Range, MaxChangeAbs> = BoundedType<Rate, Range, MaxChangeAbs>;

/// Fractional rate.
///
/// The range is between 0 to 1, and max absolute value of change diff is 1/5.
pub type FractionalRate = BoundedRate<Fractional, OneFifth>;

pub type BoundedBalance<Range, MaxChangeAbs> = BoundedType<Balance, Range, MaxChangeAbs>;

pub type BoundedBlockNumber<Range, MaxChangeAbs> = BoundedType<BlockNumber, Range, MaxChangeAbs>;

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::{assert_err, assert_ok};

	#[test]
	fn fractional_rate_works() {
		assert_err!(FractionalRate::try_from(Rate::from_rational(11, 10)), Error::OutOfBound);

		let mut rate = FractionalRate::try_from(Rate::from_rational(8, 10)).unwrap();
		assert_ok!(rate.set(Rate::from_rational(10, 10)));
		assert_err!(rate.set(Rate::from_rational(11, 10)), Error::OutOfBound);
		assert_err!(rate.set(Rate::from_rational(79, 100)), Error::ExceedMaxChangeAbs);

		assert_eq!(FractionalRate::default().get(), Rate::zero());
	}

	#[test]
	fn bounded_type_default_is_range_min() {
		#[derive(Clone, Copy, PartialEq, Eq, RuntimeDebug)]
		pub struct OneToTwo;
		impl Get<(Rate, Rate)> for OneToTwo {
			fn get() -> (Rate, Rate) {
				(Rate::one(), Rate::from_rational(2, 1))
			}
		}

		type BoundedRateOneToTwo = BoundedRate<OneToTwo, OneFifth>;

		assert_eq!(BoundedRateOneToTwo::default().get(), Rate::one());
	}
}
