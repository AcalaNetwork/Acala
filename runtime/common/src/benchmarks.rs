// This file is part of Acala.

// Copyright (C) 2020-2025 Acala Foundation.
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

//! Common runtime benchmarking code.

use crate::{dollar, parameter_types, AccountId, Balance, CurrencyId, Price, ACA, AUSD, DOT};
use frame_support::PalletId;
use primitives::AuthoritysOriginId;
use sp_core::Get;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::{FixedPointNumber, FixedU128};
use sp_std::vec;

/// Helper struct for benchmarking.
pub struct BenchmarkHelper<T>(sp_std::marker::PhantomData<T>);

/// Instance helper struct for benchmarking.
pub struct BenchmarkInstanceHelper<T, I>(sp_std::marker::PhantomData<(T, I)>);

impl<T, I> orml_oracle::BenchmarkHelper<T::OracleKey, T::OracleValue, T::MaxFeedValues>
	for BenchmarkInstanceHelper<T, I>
where
	T: orml_oracle::Config<I, OracleKey = CurrencyId, OracleValue = Price>,
{
	fn get_currency_id_value_pairs() -> sp_runtime::BoundedVec<(T::OracleKey, T::OracleValue), T::MaxFeedValues> {
		sp_runtime::BoundedVec::try_from(vec![
			(DOT, FixedU128::saturating_from_rational(1, 1)),
			(ACA, FixedU128::saturating_from_rational(2, 1)),
			(AUSD, FixedU128::saturating_from_rational(3, 1)),
		])
		.unwrap()
	}
}

impl<T> orml_tokens::BenchmarkHelper<T::CurrencyId, T::Balance> for BenchmarkHelper<T>
where
	T: orml_tokens::Config<CurrencyId = CurrencyId, Balance = Balance>,
{
	fn get_currency_id_and_amount() -> Option<(T::CurrencyId, T::Balance)> {
		Some((DOT, dollar(DOT)))
	}
}

parameter_types! {
	pub const TreasuryPalletId: PalletId = PalletId(*b"aca/trsy");
}

impl<T> orml_vesting::BenchmarkHelper<T::AccountId, <T as pallet_balances::Config>::Balance> for BenchmarkHelper<T>
where
	T: frame_system::Config<AccountId = AccountId> + pallet_balances::Config<Balance = Balance> + orml_vesting::Config,
{
	fn get_vesting_account_and_amount() -> Option<(T::AccountId, <T as pallet_balances::Config>::Balance)> {
		Some((TreasuryPalletId::get().into_account_truncating(), dollar(ACA)))
	}
}

impl<T> orml_authority::BenchmarkHelper<T::AsOriginId> for BenchmarkHelper<T>
where
	T: orml_authority::Config<AsOriginId = AuthoritysOriginId>,
{
	fn get_as_origin_id() -> Option<T::AsOriginId> {
		Some(AuthoritysOriginId::Root)
	}
}
