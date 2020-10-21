//! A set of constant values used in dev runtime.

/// Money matters.
pub mod currency {
	use primitives::Balance;

	pub const DOLLARS: Balance = 1_000_000_000_000_000_000;
	pub const CENTS: Balance = DOLLARS / 100; // 10_000_000_000_000_000
	pub const MILLICENTS: Balance = CENTS / 1000; // 10_000_000_000_000
	pub const MICROCENTS: Balance = MILLICENTS / 1000; // 10_000_000_000

	pub const fn deposit(items: u32, bytes: u32) -> Balance {
		items as Balance * 15 * CENTS + (bytes as Balance) * 6 * CENTS
	}
}

/// Time and blocks.
pub mod time {
	use primitives::{BlockNumber, Moment};

	pub const SECS_PER_BLOCK: Moment = 4;
	pub const MILLISECS_PER_BLOCK: Moment = SECS_PER_BLOCK * 1000;

	// These time units are defined in number of blocks.
	pub const MINUTES: BlockNumber = 60 / (SECS_PER_BLOCK as BlockNumber);
	pub const HOURS: BlockNumber = MINUTES * 60;
	pub const DAYS: BlockNumber = HOURS * 24;

	pub const SLOT_DURATION: Moment = MILLISECS_PER_BLOCK;

	// 1 in 4 blocks (on average, not counting collisions) will be primary BABE
	// blocks.
	pub const PRIMARY_PROBABILITY: (u64, u64) = (1, 4);

	pub const EPOCH_DURATION_IN_BLOCKS: BlockNumber = HOURS;
	pub const EPOCH_DURATION_IN_SLOTS: u64 = {
		const SLOT_FILL_RATE: f64 = MILLISECS_PER_BLOCK as f64 / SLOT_DURATION as f64;

		(EPOCH_DURATION_IN_BLOCKS as f64 * SLOT_FILL_RATE) as u64
	};
}

/// Fee-related
pub mod fee {
	pub use super::currency::CENTS;
	use frame_support::weights::{
		constants::ExtrinsicBaseWeight, WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
	};
	use primitives::Balance;
	use smallvec::smallvec;
	use sp_runtime::Perbill;

	/// The block saturation level. Fees will be updates based on this value.
	pub const TARGET_BLOCK_FULLNESS: Perbill = Perbill::from_percent(25);

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
	///   - Setting it to `1` will cause the literal `#[weight = x]` values to
	///     be charged.
	pub struct WeightToFee;
	impl WeightToFeePolynomial for WeightToFee {
		type Balance = Balance;
		fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
			// in Acala, extrinsic base weight (smallest non-zero weight) is mapped to 1/10
			// CENT:
			let p = CENTS / 10; // 1_000_000_000_000_000
			let q = Balance::from(ExtrinsicBaseWeight::get()); // 125_000_000
			smallvec![WeightToFeeCoefficient {
				degree: 1,
				negative: false,
				coeff_frac: Perbill::from_rational_approximation(p % q, q), // zero
				coeff_integer: p / q,                                       // 8_000_000
			}]
		}
	}
}
