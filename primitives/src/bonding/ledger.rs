// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
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

use super::error::Error;
use crate::Balance;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{traits::Zero, RuntimeDebug};

use frame_support::pallet_prelude::*;

/// Just a Balance/BlockNumber tuple to encode when a chunk of funds will be
/// unlocked.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct UnlockChunk<Moment> {
	/// Amount of funds to be unlocked.
	value: Balance,
	/// Era number at which point it'll be unlocked.
	unlock_at: Moment,
}

/// The ledger of a (bonded) account.
#[derive(PartialEqNoBound, EqNoBound, CloneNoBound, Encode, Decode, RuntimeDebug, MaxEncodedLen, TypeInfo)]
#[scale_info(skip_type_params(MaxUnlockingChunks, MinBond))]
pub struct BondingLedger<Moment, MaxUnlockingChunks, MinBond>
where
	Moment: Eq + Clone,
	MaxUnlockingChunks: Get<u32>,
{
	/// The total amount of the account's balance that we are currently
	/// accounting for. It's just `active` plus all the `unlocking`
	/// balances.
	total: Balance,
	/// The total amount of the account's balance that will be at stake in
	/// any forthcoming rounds.
	active: Balance,
	/// Any balance that is becoming free, which may eventually be
	/// transferred out of the account.
	unlocking: BoundedVec<UnlockChunk<Moment>, MaxUnlockingChunks>,

	_phantom: PhantomData<MinBond>,
}

impl<Moment, MaxUnlockingChunks, MinBond> BondingLedger<Moment, MaxUnlockingChunks, MinBond>
where
	Moment: Ord + Eq + Copy,
	MaxUnlockingChunks: Get<u32>,
	MinBond: Get<Balance>,
{
	pub fn new() -> Self {
		Default::default()
	}

	pub fn active(&self) -> Balance {
		self.active
	}

	pub fn total(&self) -> Balance {
		self.total
	}

	pub fn unlocking(&self) -> sp_std::vec::Vec<(Balance, Moment)> {
		self.unlocking
			.iter()
			.cloned()
			.map(|chunk| (chunk.value, chunk.unlock_at))
			.collect()
	}

	pub fn unlocking_len(&self) -> usize {
		self.unlocking.len()
	}

	/// Bond more funds.
	pub fn bond(mut self, amount: Balance) -> Result<Self, Error> {
		self.active = self.active.saturating_add(amount);
		self.total = self.total.saturating_add(amount);
		self.check_min_bond()?;
		Ok(self)
	}

	/// Start unbonding and create new UnlockChunk.
	/// Note that if the `unlock_at` is same as the last UnlockChunk, they will be merged.
	pub fn unbond(mut self, amount: Balance, unlock_at: Moment) -> Result<(Self, Balance), Error> {
		let amount = amount.min(self.active);
		self.active = self.active.saturating_sub(amount);
		self.check_min_bond()?;
		self.unlocking = self
			.unlocking
			.try_mutate(|unlocking| {
				// try merge if the last chunk unlock time is same
				if let Some(last) = unlocking.last_mut() {
					if last.unlock_at == unlock_at {
						last.value = last.value.saturating_add(amount);
						return;
					}
				}
				// or make a new one
				unlocking.push(UnlockChunk {
					value: amount,
					unlock_at,
				});
			})
			.ok_or(Error::MaxUnlockChunksExceeded)?;
		Ok((self, amount))
	}

	pub fn unbond_instant(mut self, amount: Balance) -> Result<(Self, Balance), Error> {
		let amount = amount.min(self.active);
		self.active = self.active.saturating_sub(amount);
		self.total = self.total.saturating_sub(amount);
		self.check_min_bond()?;
		Ok((self, amount))
	}

	/// Remove entries from `unlocking` that are sufficiently old and reduce
	/// the total by the sum of their balances.
	#[must_use]
	pub fn consolidate_unlocked(mut self, now: Moment) -> Self {
		let mut total = self.total;
		self.unlocking.retain(|chunk| {
			if chunk.unlock_at > now {
				true
			} else {
				total = total.saturating_sub(chunk.value);
				false
			}
		});

		self.total = total;

		self
	}

	/// Re-bond funds that were scheduled for unlocking.
	pub fn rebond(mut self, value: Balance) -> Result<(Self, Balance), Error> {
		let mut unlocking_balance: Balance = Zero::zero();

		self.unlocking = self
			.unlocking
			.try_mutate(|unlocking| {
				while let Some(last) = unlocking.last_mut() {
					if unlocking_balance + last.value <= value {
						unlocking_balance += last.value;
						self.active += last.value;
						unlocking.pop();
					} else {
						let diff = value - unlocking_balance;

						unlocking_balance += diff;
						self.active += diff;
						last.value -= diff;
					}

					if unlocking_balance >= value {
						break;
					}
				}
			})
			.expect("Only popped elements from inner_vec");

		self.check_min_bond()?;

		Ok((self, unlocking_balance))
	}

	pub fn is_empty(&self) -> bool {
		self.total.is_zero()
	}

	fn check_min_bond(&self) -> Result<(), Error> {
		if self.active > 0 && self.active < MinBond::get() {
			return Err(Error::BelowMinBondThreshold);
		}
		Ok(())
	}
}

impl<Moment, MaxUnlockingChunks, MinBond> Default for BondingLedger<Moment, MaxUnlockingChunks, MinBond>
where
	Moment: Ord + Eq + Copy,
	MaxUnlockingChunks: Get<u32>,
	MinBond: Get<Balance>,
{
	fn default() -> Self {
		Self {
			unlocking: Default::default(),
			total: Default::default(),
			active: Default::default(),
			_phantom: Default::default(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::{
		assert_err,
		traits::{ConstU128, ConstU32},
	};
	use sp_runtime::bounded_vec;

	type Ledger = BondingLedger<u32, ConstU32<3>, ConstU128<10>>;

	#[test]
	fn bond_works() {
		let ledger = Ledger::new();
		assert!(ledger.is_empty());
		assert_err!(ledger.clone().bond(9), Error::BelowMinBondThreshold);

		let ledger = ledger.bond(10).unwrap();
		assert!(!ledger.is_empty());
		assert_eq!(
			ledger,
			Ledger {
				total: 10,
				active: 10,
				unlocking: Default::default(),
				_phantom: Default::default(),
			}
		);

		let ledger = ledger.bond(100).unwrap();
		assert_eq!(
			ledger,
			Ledger {
				total: 110,
				active: 110,
				unlocking: Default::default(),
				_phantom: Default::default(),
			}
		);
	}

	#[test]
	fn unbond_works() {
		let ledger = Ledger::new();
		let ledger = ledger.bond(100).unwrap();
		assert_err!(ledger.clone().unbond(99, 2), Error::BelowMinBondThreshold);

		let (ledger, actual) = ledger.unbond(20, 2).unwrap();
		assert_eq!(actual, 20);
		assert_eq!(
			ledger,
			Ledger {
				total: 100,
				active: 80,
				unlocking: bounded_vec![UnlockChunk {
					value: 20,
					unlock_at: 2,
				}],
				_phantom: Default::default(),
			}
		);

		let (ledger, actual) = ledger.unbond(10, 2).unwrap();
		assert_eq!(actual, 10);
		assert_eq!(
			ledger,
			Ledger {
				total: 100,
				active: 70,
				unlocking: bounded_vec![UnlockChunk {
					value: 30,
					unlock_at: 2,
				}],
				_phantom: Default::default(),
			}
		);

		let (ledger, actual) = ledger.unbond(5, 4).unwrap();
		assert_eq!(actual, 5);
		assert_eq!(
			ledger,
			Ledger {
				total: 100,
				active: 65,
				unlocking: bounded_vec![
					UnlockChunk {
						value: 30,
						unlock_at: 2,
					},
					UnlockChunk { value: 5, unlock_at: 4 }
				],
				_phantom: Default::default(),
			}
		);

		let ledger = ledger.consolidate_unlocked(1);
		assert_eq!(
			ledger,
			Ledger {
				total: 100,
				active: 65,
				unlocking: bounded_vec![
					UnlockChunk {
						value: 30,
						unlock_at: 2,
					},
					UnlockChunk { value: 5, unlock_at: 4 }
				],
				_phantom: Default::default(),
			}
		);

		let ledger = ledger.consolidate_unlocked(2);
		assert_eq!(
			ledger,
			Ledger {
				total: 70,
				active: 65,
				unlocking: bounded_vec![UnlockChunk { value: 5, unlock_at: 4 }],
				_phantom: Default::default(),
			}
		);

		let (ledger, actual) = ledger.unbond(100, 6).unwrap();
		assert_eq!(actual, 65);
		assert_eq!(
			ledger,
			Ledger {
				total: 70,
				active: 0,
				unlocking: bounded_vec![
					UnlockChunk { value: 5, unlock_at: 4 },
					UnlockChunk {
						value: 65,
						unlock_at: 6,
					}
				],
				_phantom: Default::default(),
			}
		);

		let ledger = ledger.consolidate_unlocked(4);
		assert_eq!(
			ledger,
			Ledger {
				total: 65,
				active: 0,
				unlocking: bounded_vec![UnlockChunk {
					value: 65,
					unlock_at: 6,
				}],
				_phantom: Default::default(),
			}
		);

		let ledger = ledger.consolidate_unlocked(6);
		assert_eq!(
			ledger,
			Ledger {
				total: 0,
				active: 0,
				unlocking: bounded_vec![],
				_phantom: Default::default(),
			}
		);
		assert!(ledger.is_empty());
	}

	#[test]
	fn unbond_instant_works() {
		let ledger = Ledger::new();
		let ledger = ledger.bond(100).unwrap();
		assert_err!(ledger.clone().unbond_instant(99), Error::BelowMinBondThreshold);

		let (ledger, actual) = ledger.unbond_instant(20).unwrap();
		assert_eq!(actual, 20);

		let (_ledger, actual) = ledger.unbond_instant(100).unwrap();
		assert_eq!(actual, 80);
	}

	#[test]
	fn rebond_works() {
		let ledger = Ledger::new();

		let (ledger, _) = ledger
			.bond(100)
			.and_then(|ledger| ledger.unbond(50, 2))
			.and_then(|(ledger, _)| ledger.unbond(50, 3))
			.unwrap();

		assert_err!(ledger.clone().rebond(1), Error::BelowMinBondThreshold);

		let (ledger, actual) = ledger.rebond(20).unwrap();
		assert_eq!(actual, 20);
		assert_eq!(
			ledger,
			Ledger {
				total: 100,
				active: 20,
				unlocking: bounded_vec![
					UnlockChunk {
						value: 50,
						unlock_at: 2
					},
					UnlockChunk {
						value: 30,
						unlock_at: 3
					}
				],
				_phantom: Default::default(),
			}
		);

		let (ledger, actual) = ledger.rebond(40).unwrap();
		assert_eq!(actual, 40);
		assert_eq!(
			ledger,
			Ledger {
				total: 100,
				active: 60,
				unlocking: bounded_vec![UnlockChunk {
					value: 40,
					unlock_at: 2
				}],
				_phantom: Default::default(),
			}
		);

		let (ledger, actual) = ledger.rebond(50).unwrap();
		assert_eq!(actual, 40);
		assert_eq!(
			ledger,
			Ledger {
				total: 100,
				active: 100,
				unlocking: bounded_vec![],
				_phantom: Default::default(),
			}
		);
	}
}
