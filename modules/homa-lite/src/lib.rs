// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

pub mod benchmarking;
mod mock;
mod tests;
pub mod weights;

use frame_support::{pallet_prelude::*, transactional};
use frame_system::{ensure_signed, pallet_prelude::*};
use module_support::{ExchangeRate, Ratio};
use orml_traits::{MultiCurrency, XcmTransfer};
use primitives::{Balance, CurrencyId};
use sp_runtime::{traits::Zero, ArithmeticError, FixedPointNumber, Permill};
use sp_std::{ops::Mul, prelude::*};
use xcm::opaque::v0::{MultiLocation, Outcome};

pub use module::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;

		/// Multi-currency support for asset management
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

		/// The Currency ID for the Staking asset
		#[pallet::constant]
		type StakingCurrencyId: Get<CurrencyId>;

		/// The Currency ID for the Liquid asset
		#[pallet::constant]
		type LiquidCurrencyId: Get<CurrencyId>;

		/// Origin represented Governance
		type GovernanceOrigin: EnsureOrigin<Self::Origin>;

		/// The minimal amount of Staking currency to be locked
		#[pallet::constant]
		type MinimumMintThreshold: Get<Balance>;

		/// The interface to Cross-chain transfer.
		type XcmTransfer: XcmTransfer<Self::AccountId, Balance, CurrencyId>;

		/// The sovereign sub-account for where the staking currencies are sent to.
		#[pallet::constant]
		type SovereignSubAccountLocation: Get<MultiLocation>;

		/// The default exchange rate for liquid currency to staking currency.
		#[pallet::constant]
		type DefaultExchangeRate: Get<ExchangeRate>;

		/// The maximum rewards that are earned on the relaychain.
		#[pallet::constant]
		type MaxRewardPerEra: Get<Permill>;

		/// The fixed cost of transaction fee for XCM transfers.
		#[pallet::constant]
		type MintFee: Get<Balance>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The total amount for the Staking currency must be more than zero.
		InvalidTotalStakingCurrency,
		/// The mint amount is below the minimum threshold allowed.
		MintAmountBelowMinimumThreshold,
		/// The amount of Staking currency used has exceeded the cap allowed.
		ExceededStakingCurrencyMintCap,
		/// Error has occurred during Cross-chain transfer.
		XcmTransferFailed,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	#[pallet::metadata(T::AccountId = "AccountId")]
	pub enum Event<T: Config> {
		/// The user has requested some Staking currency to be used to mint Liquid Currency.
		/// \[user, amount_staked, amount_minted\]
		Minted(T::AccountId, Balance, Balance),

		/// The total amount of the staking currency on the relaychain has been
		/// set.\[total_staking_currency\]
		TotalStakingCurrencySet(Balance),

		/// The mint cap for Staking currency is updated.\[new_cap\]
		StakingCurrencyMintCapUpdated(Balance),

		/// A new weight for XCM transfers has been set.\[new_weight\]
		XcmDestWeightSet(Weight),
	}

	/// The total amount of the staking currency on the relaychain.
	/// This info is used to calculate the exchange rate between Staking and Liquid currencies.
	/// TotalStakingCurrency: value: Balance
	#[pallet::storage]
	#[pallet::getter(fn total_staking_currency)]
	pub type TotalStakingCurrency<T: Config> = StorageValue<_, Balance, ValueQuery>;

	/// The cap on the total amount of staking currency allowed to mint Liquid currency.
	/// StakingCurrencyMintCap: value: Balance
	#[pallet::storage]
	#[pallet::getter(fn staking_currency_mint_cap)]
	pub type StakingCurrencyMintCap<T: Config> = StorageValue<_, Balance, ValueQuery>;

	/// The extra weight for cross-chain XCM transfers.
	/// xcm_dest_weight: value: Weight
	#[pallet::storage]
	#[pallet::getter(fn xcm_dest_weight)]
	pub type XcmDestWeight<T: Config> = StorageValue<_, Weight, ValueQuery>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Mint some Liquid currency, by locking up the given amount of Staking currency.
		/// The exchange rate is calculated using the ratio of the total amount of the staking and
		/// liquid currency. A portion is reducted (defined as T::MaxRewardPerEra) to make up for
		/// the fact that staking is only effective from the next era on (on the relaychain).
		///
		/// Parameters:
		/// - `amount`: The amount of Staking currency to be exchanged.
		#[pallet::weight(< T as Config >::WeightInfo::mint())]
		#[transactional]
		pub fn mint(origin: OriginFor<T>, amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;
			// Ensure the amount is above the minimum, after the MintFee is deducted.
			ensure!(
				amount > T::MinimumMintThreshold::get().saturating_add(T::MintFee::get()),
				Error::<T>::MintAmountBelowMinimumThreshold
			);

			// Ensure the total amount staked doesn't exceed the cap.
			let new_total_staked = Self::total_staking_currency()
				.checked_add(amount)
				.ok_or(ArithmeticError::Overflow)?;
			ensure!(
				new_total_staked <= Self::staking_currency_mint_cap(),
				Error::<T>::ExceededStakingCurrencyMintCap
			);

			let staking_currency = T::StakingCurrencyId::get();

			// ensure the user has enough funds on their account.
			T::Currency::ensure_can_withdraw(staking_currency, &who, amount)?;

			// Calculate how much Liquid currency is to be minted.
			// Gets the current exchange rate
			let staking_total = Self::total_staking_currency();
			let liquid_total = T::Currency::total_issuance(T::LiquidCurrencyId::get());
			let exchange_rate =
				Ratio::checked_from_rational(liquid_total, staking_total).unwrap_or_else(T::DefaultExchangeRate::get);

			// liquid_to_mint = ( (staked_amount - MintFee) * liquid_total / staked_total ) * (1 -
			// MaxRewardPerEra)
			let mut liquid_to_mint = exchange_rate
				.checked_mul_int(
					amount
						.checked_sub(T::MintFee::get())
						.expect("Mint amount is ensured to be greater than T::MintFee; qed"),
				)
				.ok_or(ArithmeticError::Overflow)?;

			liquid_to_mint = liquid_to_mint
				.checked_sub(T::MaxRewardPerEra::get().mul(liquid_to_mint))
				.expect("Max rewards cannot be above 100%; qed");

			// All checks pass. Proceed with Xcm transfer.
			let xcm_result = T::XcmTransfer::transfer(
				who.clone(),
				staking_currency,
				amount,
				T::SovereignSubAccountLocation::get(),
				Self::xcm_dest_weight(),
			)?;
			ensure!(
				matches!(xcm_result, Outcome::Complete(_)),
				Error::<T>::XcmTransferFailed
			);

			// Mint the liquid currency into the user's account.
			T::Currency::deposit(T::LiquidCurrencyId::get(), &who, liquid_to_mint)?;

			TotalStakingCurrency::<T>::put(new_total_staked);

			Self::deposit_event(Event::<T>::Minted(who, amount, liquid_to_mint));

			Ok(())
		}

		/// Sets the total amount of the Staking currency that are currently on the relaychain.
		/// Requires `T::GovernanceOrigin`
		///
		/// Parameters:
		/// - `staking_total`: The current amount of the Staking currency. Used to calculate
		///   conversion rate.
		#[pallet::weight(< T as Config >::WeightInfo::set_total_staking_currency())]
		#[transactional]
		pub fn set_total_staking_currency(origin: OriginFor<T>, staking_total: Balance) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;
			ensure!(!staking_total.is_zero(), Error::<T>::InvalidTotalStakingCurrency);

			TotalStakingCurrency::<T>::put(staking_total);
			Self::deposit_event(Event::<T>::TotalStakingCurrencySet(staking_total));

			Ok(())
		}

		/// Updates the cap for how much Staking currency can be used to Mint liquid currency.
		/// Requires `T::GovernanceOrigin`
		///
		/// Parameters:
		/// - `new_cap`: The new cap for staking currency.
		#[pallet::weight(< T as Config >::WeightInfo::set_minting_cap())]
		#[transactional]
		pub fn set_minting_cap(origin: OriginFor<T>, new_cap: Balance) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			StakingCurrencyMintCap::<T>::put(new_cap);
			Self::deposit_event(Event::<T>::StakingCurrencyMintCapUpdated(new_cap));
			Ok(())
		}

		/// Sets the xcm_dest_weight for XCM transfers.
		/// Requires `T::GovernanceOrigin`
		///
		/// Parameters:
		/// - `xcm_dest_weight`: The new weight for XCM transfers.
		#[pallet::weight(< T as Config >::WeightInfo::set_xcm_dest_weight())]
		#[transactional]
		pub fn set_xcm_dest_weight(origin: OriginFor<T>, xcm_dest_weight: Weight) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			XcmDestWeight::<T>::put(xcm_dest_weight);
			Self::deposit_event(Event::<T>::XcmDestWeightSet(xcm_dest_weight));
			Ok(())
		}
	}
}
