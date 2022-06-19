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

//! A set of constant values used in Karura runtime.

/// Time and blocks.
pub mod time {
	use primitives::{Balance, BlockNumber, Moment};
	use runtime_common::{dollar, millicent, KAR};

	pub const SECS_PER_BLOCK: Moment = 12;
	pub const MILLISECS_PER_BLOCK: Moment = SECS_PER_BLOCK * 1000;

	// These time units are defined in number of blocks.
	pub const MINUTES: BlockNumber = 60 / (SECS_PER_BLOCK as BlockNumber);
	pub const HOURS: BlockNumber = MINUTES * 60;
	pub const DAYS: BlockNumber = HOURS * 24;

	pub const SLOT_DURATION: Moment = MILLISECS_PER_BLOCK;

	pub fn deposit(items: u32, bytes: u32) -> Balance {
		items as Balance * 2 * dollar(KAR) + (bytes as Balance) * 30 * millicent(KAR)
	}
}

/// Fee-related
pub mod fee {
	use frame_support::weights::{
		constants::{ExtrinsicBaseWeight, WEIGHT_PER_SECOND},
		WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
	};
	use primitives::Balance;
	use runtime_common::{cent, KAR};
	use smallvec::smallvec;
	use sp_runtime::Perbill;

	pub fn base_tx_in_kar() -> Balance {
		cent(KAR) / 10
	}

	/// Handles converting a weight scalar to a fee value, based on the scale
	/// and granularity of the node's balance type.
	///
	/// This should typically create a mapping between the following ranges:
	///   - [0, system::MaximumBlockWeight]
	///   - [Balance::min, Balance::max]
	///
	/// Yet, it can be used for any other sort of change to weight-fee. Some
	/// examples being:
	///   - Setting it to `0` will essentially disable the weight fee.
	///   - Setting it to `1` will cause the literal `#[weight = x]` values to be charged.
	pub struct WeightToFee;
	impl WeightToFeePolynomial for WeightToFee {
		type Balance = Balance;
		fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
			// in Karura, extrinsic base weight (smallest non-zero weight) is mapped to 1/10 CENT:
			let p = base_tx_in_kar();
			let q = Balance::from(ExtrinsicBaseWeight::get());
			smallvec![WeightToFeeCoefficient {
				degree: 1,
				negative: false,
				coeff_frac: Perbill::from_rational(p % q, q),
				coeff_integer: p / q,
			}]
		}
	}

	pub fn kar_per_second() -> u128 {
		let base_weight = Balance::from(ExtrinsicBaseWeight::get());
		let base_tx_per_second = (WEIGHT_PER_SECOND as u128) / base_weight;
		base_tx_per_second * base_tx_in_kar()
	}

	pub fn ksm_per_second() -> u128 {
		kar_per_second() / 50
	}
}

pub mod parachains {
	pub mod bifrost {
		pub const ID: u32 = 2001;
		pub const BNC_KEY: &[u8] = &[0, 1];
		pub const VSKSM_KEY: &[u8] = &[4, 4];
	}

	pub mod phala {
		pub const ID: u32 = 2004;
	}

	pub mod kintsugi {
		pub const ID: u32 = 2092;
		pub const KBTC_KEY: &[u8] = &[0, 11];
		pub const KINT_KEY: &[u8] = &[0, 12];
	}

	pub mod statemine {
		pub const ID: u32 = 1000;
	}
}

#[cfg(test)]
mod tests {
	use crate::{constants::fee::base_tx_in_kar, Balance};
	use frame_support::weights::constants::ExtrinsicBaseWeight;

	#[test]
	fn check_weight() {
		let p = base_tx_in_kar();
		let q = Balance::from(ExtrinsicBaseWeight::get());

		assert_eq!(p, 1_000_000_000);
		assert_eq!(q, 85_795_000);
	}
}
