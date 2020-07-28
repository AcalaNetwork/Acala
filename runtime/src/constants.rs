//! A set of constant values used in runtime.

/// Money matters.
pub mod currency {
	use crate::types::Balance;

	pub const DOLLARS: Balance = 1_000_000_000_000_000_000;
	pub const CENTS: Balance = DOLLARS / 100; // 10_000_000_000_000_000
	pub const MILLICENTS: Balance = CENTS / 1000; // 10_000_000_000_000
	pub const MICROCENTS: Balance = MILLICENTS / 1000; // 10_000_000_000
}

/// Time.
pub mod time {
	use crate::types::{BlockNumber, Moment};

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
