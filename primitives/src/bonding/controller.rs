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

use frame_support::{dispatch::DispatchResult, pallet_prelude::Member, traits::Get, Parameter, StorageMap};
use parity_scale_codec::Codec;
use sp_runtime::DispatchError;
use sp_std::prelude::*;

use super::error::Error;
use super::ledger::{BondingLedger, UnlockChunk};
use crate::Balance;

pub type BondingLedgerOf<T> = BondingLedger<
	<T as BondingController>::Moment,
	<T as BondingController>::MaxUnbondingChunks,
	<T as BondingController>::MinBond,
>;

pub struct BondChange {
	pub new: Balance,
	pub old: Balance,
	pub change: Balance,
}
pub trait BondingController
where
	BondingLedgerOf<Self>: Codec + Default,
	frame_support::BoundedVec<
		UnlockChunk<<Self as BondingController>::Moment>,
		<Self as BondingController>::MaxUnbondingChunks,
	>: Codec,
{
	type MinBond: Get<Balance>;
	type MaxUnbondingChunks: Get<u32>;
	type Moment: Ord + Eq + Copy;
	type AccountId: Parameter + Member;
	type Ledger: StorageMap<Self::AccountId, BondingLedgerOf<Self>, Query = Option<BondingLedgerOf<Self>>>;

	fn available_balance(who: &Self::AccountId, ledger: &BondingLedgerOf<Self>) -> Balance;
	fn apply_ledger(who: &Self::AccountId, ledger: &BondingLedgerOf<Self>) -> DispatchResult;
	fn convert_error(err: Error) -> DispatchError;

	fn bond(who: &Self::AccountId, amount: Balance) -> Result<Option<BondChange>, DispatchError> {
		let ledger = Self::Ledger::get(who).unwrap_or_default();

		let available = Self::available_balance(who, &ledger);
		let bond_amount = amount.min(available);

		if bond_amount == 0 {
			return Ok(None);
		}

		let old_active = ledger.active();

		let ledger = ledger.bond(bond_amount).map_err(Self::convert_error)?;

		Self::Ledger::insert(who, &ledger);
		Self::apply_ledger(who, &ledger)?;

		Ok(Some(BondChange {
			old: old_active,
			new: ledger.active(),
			change: bond_amount,
		}))
	}

	fn unbond(who: &Self::AccountId, amount: Balance, at: Self::Moment) -> Result<Option<BondChange>, DispatchError> {
		let ledger = Self::Ledger::get(who).ok_or_else(|| Self::convert_error(Error::NotBonded))?;
		let old_active = ledger.active();

		let (ledger, unbond_amount) = ledger.unbond(amount, at).map_err(Self::convert_error)?;

		if unbond_amount == 0 {
			return Ok(None);
		}

		Self::Ledger::insert(who, &ledger);
		Self::apply_ledger(who, &ledger)?;

		Ok(Some(BondChange {
			old: old_active,
			new: ledger.active(),
			change: unbond_amount,
		}))
	}

	fn unbond_instant(who: &Self::AccountId, amount: Balance) -> Result<Option<BondChange>, DispatchError> {
		let ledger = Self::Ledger::get(who).ok_or_else(|| Self::convert_error(Error::NotBonded))?;
		let old_active = ledger.active();

		let (ledger, unbond_amount) = ledger.unbond_instant(amount).map_err(Self::convert_error)?;

		if unbond_amount == 0 {
			return Ok(None);
		}

		Self::Ledger::insert(who, &ledger);
		Self::apply_ledger(who, &ledger)?;

		Ok(Some(BondChange {
			old: old_active,
			new: ledger.active(),
			change: unbond_amount,
		}))
	}

	fn rebond(who: &Self::AccountId, amount: Balance) -> Result<Option<BondChange>, DispatchError> {
		let ledger = Self::Ledger::get(who).ok_or_else(|| Self::convert_error(Error::NotBonded))?;
		let old_active = ledger.active();

		let (ledger, rebond_amount) = ledger.rebond(amount).map_err(Self::convert_error)?;

		if rebond_amount == 0 {
			return Ok(None);
		}

		Self::Ledger::insert(who, &ledger);
		Self::apply_ledger(who, &ledger)?;

		Ok(Some(BondChange {
			old: old_active,
			new: ledger.active(),
			change: rebond_amount,
		}))
	}

	fn withdraw_unbonded(who: &Self::AccountId, now: Self::Moment) -> Result<Option<BondChange>, DispatchError> {
		let ledger = Self::Ledger::get(who).ok_or_else(|| Self::convert_error(Error::NotBonded))?;
		let old_total = ledger.total();

		let ledger = ledger.consolidate_unlocked(now);

		let new_total = ledger.total();

		let diff = old_total.saturating_sub(new_total);

		if diff == 0 {
			return Ok(None);
		}

		if new_total == 0 {
			Self::Ledger::remove(who);
		} else {
			Self::Ledger::insert(who, &ledger);
		}

		Self::apply_ledger(who, &ledger)?;

		Ok(Some(BondChange {
			old: old_total,
			new: new_total,
			change: diff,
		}))
	}
}
