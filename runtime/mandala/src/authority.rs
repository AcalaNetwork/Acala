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

//! An orml_authority trait implementation.

use crate::{
	AccountId, AccountIdConversion, AuthoritysOriginId, BadOrigin, BlockNumber, DispatchResult, EnsureRoot,
	EnsureRootOrHalfFinancialCouncil, EnsureRootOrHalfGeneralCouncil, EnsureRootOrHalfHomaCouncil,
	EnsureRootOrOneThirdsTechnicalCommittee, EnsureRootOrThreeFourthsGeneralCouncil,
	EnsureRootOrTwoThirdsTechnicalCommittee, HomaTreasuryPalletId, HonzonTreasuryPalletId, OneDay, OriginCaller,
	RuntimeOrigin, SevenDays, TreasuryPalletId, TreasuryReservePalletId, ZeroDay, HOURS,
};
pub use frame_support::traits::{schedule::Priority, EnsureOrigin, OriginTrait};
use frame_system::ensure_root;
use orml_authority::EnsureDelayed;
use sp_std::cmp::Ordering;

pub struct AuthorityConfigImpl;
impl orml_authority::AuthorityConfig<RuntimeOrigin, OriginCaller, BlockNumber> for AuthorityConfigImpl {
	fn check_schedule_dispatch(origin: RuntimeOrigin, _priority: Priority) -> DispatchResult {
		EnsureRoot::<AccountId>::try_origin(origin)
			.or_else(|o| EnsureRootOrHalfGeneralCouncil::try_origin(o).map(|_| ()))
			.or_else(|o| EnsureRootOrHalfFinancialCouncil::try_origin(o).map(|_| ()))
			.or_else(|o| EnsureRootOrHalfHomaCouncil::try_origin(o).map(|_| ()))
			.map_or_else(|_| Err(BadOrigin.into()), |_| Ok(()))
	}

	fn check_fast_track_schedule(
		origin: RuntimeOrigin,
		_initial_origin: &OriginCaller,
		new_delay: BlockNumber,
	) -> DispatchResult {
		ensure_root(origin.clone()).or_else(|_| {
			if new_delay / HOURS < 12 {
				EnsureRootOrTwoThirdsTechnicalCommittee::ensure_origin(origin)
					.map_or_else(|e| Err(e.into()), |_| Ok(()))
			} else {
				EnsureRootOrOneThirdsTechnicalCommittee::ensure_origin(origin)
					.map_or_else(|e| Err(e.into()), |_| Ok(()))
			}
		})
	}

	fn check_delay_schedule(origin: RuntimeOrigin, _initial_origin: &OriginCaller) -> DispatchResult {
		ensure_root(origin.clone()).or_else(|_| {
			EnsureRootOrOneThirdsTechnicalCommittee::ensure_origin(origin).map_or_else(|e| Err(e.into()), |_| Ok(()))
		})
	}

	fn check_cancel_schedule(origin: RuntimeOrigin, initial_origin: &OriginCaller) -> DispatchResult {
		if matches!(
			cmp_privilege(origin.caller(), initial_origin),
			Some(Ordering::Greater) | Some(Ordering::Equal)
		) || EnsureRootOrThreeFourthsGeneralCouncil::ensure_origin(origin).is_ok()
		{
			Ok(())
		} else {
			Err(BadOrigin.into())
		}
	}
}

impl orml_authority::AsOriginId<RuntimeOrigin, OriginCaller> for AuthoritysOriginId {
	fn into_origin(self) -> OriginCaller {
		match self {
			AuthoritysOriginId::Root => RuntimeOrigin::root().caller().clone(),
			AuthoritysOriginId::Treasury => RuntimeOrigin::signed(TreasuryPalletId::get().into_account_truncating())
				.caller()
				.clone(),
			AuthoritysOriginId::HonzonTreasury => {
				RuntimeOrigin::signed(HonzonTreasuryPalletId::get().into_account_truncating())
					.caller()
					.clone()
			}
			AuthoritysOriginId::HomaTreasury => {
				RuntimeOrigin::signed(HomaTreasuryPalletId::get().into_account_truncating())
					.caller()
					.clone()
			}
			AuthoritysOriginId::TreasuryReserve => {
				RuntimeOrigin::signed(TreasuryReservePalletId::get().into_account_truncating())
					.caller()
					.clone()
			}
		}
	}

	fn check_dispatch_from(&self, origin: RuntimeOrigin) -> DispatchResult {
		ensure_root(origin.clone()).or_else(|_| match self {
			AuthoritysOriginId::Root => <EnsureDelayed<
				SevenDays,
				EnsureRootOrThreeFourthsGeneralCouncil,
				BlockNumber,
				OriginCaller,
			> as EnsureOrigin<RuntimeOrigin>>::ensure_origin(origin)
			.map_or_else(|_| Err(BadOrigin.into()), |_| Ok(())),
			AuthoritysOriginId::Treasury => {
				<EnsureDelayed<OneDay, EnsureRootOrHalfGeneralCouncil, BlockNumber, OriginCaller> as EnsureOrigin<
					RuntimeOrigin,
				>>::ensure_origin(origin)
				.map_or_else(|_| Err(BadOrigin.into()), |_| Ok(()))
			}
			AuthoritysOriginId::HonzonTreasury => {
				<EnsureDelayed<OneDay, EnsureRootOrHalfFinancialCouncil, BlockNumber, OriginCaller> as EnsureOrigin<
					RuntimeOrigin,
				>>::ensure_origin(origin)
				.map_or_else(|_| Err(BadOrigin.into()), |_| Ok(()))
			}
			AuthoritysOriginId::HomaTreasury => {
				<EnsureDelayed<OneDay, EnsureRootOrHalfHomaCouncil, BlockNumber, OriginCaller> as EnsureOrigin<
					RuntimeOrigin,
				>>::ensure_origin(origin)
				.map_or_else(|_| Err(BadOrigin.into()), |_| Ok(()))
			}
			AuthoritysOriginId::TreasuryReserve => {
				<EnsureDelayed<ZeroDay, EnsureRoot<AccountId>, BlockNumber, OriginCaller> as EnsureOrigin<
					RuntimeOrigin,
				>>::ensure_origin(origin)
				.map_or_else(|_| Err(BadOrigin.into()), |_| Ok(()))
			}
		})
	}
}

/// Compares privilages
fn cmp_privilege(left: &OriginCaller, right: &OriginCaller) -> Option<Ordering> {
	if left == right {
		return Some(Ordering::Equal);
	}

	match (left, right) {
		// Root always has privilage
		(OriginCaller::system(frame_system::RawOrigin::Root), _) => Some(Ordering::Greater),

		// Checks which one has more yes votes.
		(
			OriginCaller::GeneralCouncil(pallet_collective::RawOrigin::Members(l_yes_votes, l_count)),
			OriginCaller::GeneralCouncil(pallet_collective::RawOrigin::Members(r_yes_votes, r_count)),
		) => Some((l_yes_votes * r_count).cmp(&(r_yes_votes * l_count))),
		(
			OriginCaller::FinancialCouncil(pallet_collective::RawOrigin::Members(l_yes_votes, l_count)),
			OriginCaller::FinancialCouncil(pallet_collective::RawOrigin::Members(r_yes_votes, r_count)),
		) => Some((l_yes_votes * r_count).cmp(&(r_yes_votes * l_count))),
		(
			OriginCaller::HomaCouncil(pallet_collective::RawOrigin::Members(l_yes_votes, l_count)),
			OriginCaller::HomaCouncil(pallet_collective::RawOrigin::Members(r_yes_votes, r_count)),
		) => Some((l_yes_votes * r_count).cmp(&(r_yes_votes * l_count))),

		// For every other origin we don't care, as they are not used in schedule_dispatch
		_ => None,
	}
}
