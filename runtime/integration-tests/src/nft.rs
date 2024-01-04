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

use crate::setup::*;
use primitives::nft::{ClassProperty, Properties};

#[test]
fn test_nft_module() {
	ExtBuilder::default()
		.balances(vec![(
			AccountId::from(ALICE),
			NATIVE_CURRENCY,
			1_000 * dollar(NATIVE_CURRENCY),
		)])
		.build()
		.execute_with(|| {
			let metadata = vec![1];
			assert_eq!(
				Balances::free_balance(AccountId::from(ALICE)),
				1_000 * dollar(NATIVE_CURRENCY)
			);
			assert_eq!(Balances::reserved_balance(AccountId::from(ALICE)), 0);
			assert_ok!(NFT::create_class(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				metadata.clone(),
				Properties(ClassProperty::Transferable | ClassProperty::Burnable | ClassProperty::Mintable),
				Default::default(),
			));
			let deposit =
				Proxy::deposit(1u32) + CreateClassDeposit::get() + DataDepositPerByte::get() * (metadata.len() as u128);
			assert_eq!(
				Balances::free_balance(&NftPalletId::get().into_sub_account_truncating(0)),
				Balances::minimum_balance()
			);
			assert_eq!(
				Balances::reserved_balance(&NftPalletId::get().into_sub_account_truncating(0)),
				deposit
			);
			assert_eq!(
				Balances::free_balance(AccountId::from(ALICE)),
				1_000 * dollar(NATIVE_CURRENCY) - deposit - Balances::minimum_balance()
			);
			assert_eq!(Balances::reserved_balance(AccountId::from(ALICE)), 0);
			assert_ok!(Balances::deposit_into_existing(
				&NftPalletId::get().into_sub_account_truncating(0),
				1 * (CreateTokenDeposit::get() + DataDepositPerByte::get()) + Balances::minimum_balance()
			));
			assert_ok!(NFT::mint(
				RuntimeOrigin::signed(NftPalletId::get().into_sub_account_truncating(0)),
				MultiAddress::Id(AccountId::from(BOB)),
				0,
				metadata.clone(),
				Default::default(),
				1
			));
			assert_ok!(NFT::burn(RuntimeOrigin::signed(AccountId::from(BOB)), (0, 0)));
			assert_eq!(
				Balances::free_balance(AccountId::from(BOB)),
				CreateTokenDeposit::get() + DataDepositPerByte::get() + Balances::minimum_balance()
			);
			assert_noop!(
				NFT::destroy_class(
					RuntimeOrigin::signed(NftPalletId::get().into_sub_account_truncating(0)),
					0,
					MultiAddress::Id(AccountId::from(BOB))
				),
				pallet_proxy::Error::<Runtime>::NotFound
			);
			assert_ok!(NFT::destroy_class(
				RuntimeOrigin::signed(NftPalletId::get().into_sub_account_truncating(0)),
				0,
				MultiAddress::Id(AccountId::from(ALICE))
			));
			assert_eq!(
				Balances::free_balance(AccountId::from(BOB)),
				CreateTokenDeposit::get() + DataDepositPerByte::get() + Balances::minimum_balance()
			);
			assert_eq!(Balances::reserved_balance(AccountId::from(BOB)), 0);
			assert_eq!(
				Balances::free_balance(AccountId::from(ALICE)),
				1_000 * dollar(NATIVE_CURRENCY)
			);
			assert_eq!(Balances::reserved_balance(AccountId::from(ALICE)), 0);
		});
}
