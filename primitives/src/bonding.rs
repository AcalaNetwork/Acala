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

use crate::Balance;
use codec::{Decode, Encode, MaxEncodedLen};
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
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, MaxEncodedLen, TypeInfo)]
#[scale_info(skip_type_params(MaxUnlockingChunks, MinBondThreshold))]
pub struct BondingLedger<Moment, MaxUnlockingChunks, MinBondThreshold>
where
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

	_phantom: PhantomData<MinBondThreshold>,
}

pub enum Error {
	BelowMinBondThreshold,
	MaxUnlockChunksExceeded,
	NoBonded,
	NoUnlockChunk,
}

impl<Moment, MaxUnlockingChunks, MinBondThreshold> BondingLedger<Moment, MaxUnlockingChunks, MinBondThreshold>
where
	Moment: Ord,
	MaxUnlockingChunks: Get<u32>,
	MinBondThreshold: Get<Balance>,
{
	pub fn new() -> Self {
		Self {
			unlocking: Default::default(),
			total: Default::default(),
			active: Default::default(),
			_phantom: Default::default(),
		}
	}

	pub fn active(&self) -> Balance {
		self.active
	}

	pub fn total(&self) -> Balance {
		self.total
	}

	pub fn unlocking_len(&self) -> usize {
		self.unlocking.len()
	}

	pub fn unbond(&mut self, amount: Balance, unlock_at: Moment) -> Result<Balance, Error> {
		let amount = amount.min(self.active);
		self.active = self.active.saturating_sub(amount);
		self.check_min_bond()?;
		self.unlocking
			.try_push(UnlockChunk {
				value: amount,
				unlock_at,
			})
			.map_err(|_| Error::MaxUnlockChunksExceeded)?;
		Ok(amount)
	}

	/// Bond more funds.
	pub fn bond(&mut self, amount: Balance) -> Result<(), Error> {
		self.active = self.active.saturating_add(amount);
		self.total = self.total.saturating_add(amount);
		self.check_min_bond()
	}

	/// Remove entries from `unlocking` that are sufficiently old and reduce
	/// the total by the sum of their balances.
	pub fn consolidate_unlocked(&mut self, now: Moment) {
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
	}

	/// Re-bond funds that were scheduled for unlocking.
	pub fn rebond(mut self, value: Balance) -> Result<Self, Error> {
		if self.unlocking.is_empty() {
			return Err(Error::NoUnlockChunk);
		}

		let mut unlocking_balance: Balance = Zero::zero();
		let mut inner_vec = self.unlocking.into_inner();
		while let Some(last) = inner_vec.last_mut() {
			if unlocking_balance + last.value <= value {
				unlocking_balance += last.value;
				self.active += last.value;
				inner_vec.pop();
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

		self.unlocking = inner_vec.try_into().expect("Only popped elements from inner_vec");
		Ok(self)
	}

	pub fn is_empty(&self) -> bool {
		self.total.is_zero()
	}

	fn check_min_bond(&self) -> Result<(), Error> {
		if self.active > 0 && self.active < MinBondThreshold::get() {
			return Err(Error::BelowMinBondThreshold);
		}
		Ok(())
	}
}

impl<Moment, MaxUnlockingChunks, MinBondThreshold> Default
	for BondingLedger<Moment, MaxUnlockingChunks, MinBondThreshold>
where
	Moment: Ord,
	MaxUnlockingChunks: Get<u32>,
	MinBondThreshold: Get<Balance>,
{
	fn default() -> Self {
		Self::new()
	}
}
