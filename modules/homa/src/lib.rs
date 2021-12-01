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

//! Homa module.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{log, pallet_prelude::*, transactional, PalletId};
use frame_system::{ensure_signed, pallet_prelude::*};
use module_support::{ExchangeRate, ExchangeRateProvider, HomaSubAccountXcm, Rate, Ratio};
use orml_traits::MultiCurrency;
use pallet_staking::EraIndex;
use primitives::{Balance, CurrencyId};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{AccountIdConversion, Bounded, One, Saturating, UniqueSaturatedInto, Zero},
	ArithmeticError, FixedPointNumber,
};
use sp_std::{cmp::Ordering, convert::From, prelude::*, vec, vec::Vec};

pub use module::*;

mod mock;
mod tests;

#[frame_support::pallet]
pub mod module {
	use super::*;

	/// The subaccount's staking ledger which kept by Homa protocol
	#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo, Default)]
	pub struct StakingLedger {
		/// Corresponding to the active of the subaccount's staking ledger on relaychain
		#[codec(compact)]
		pub bonded: Balance,
		/// Corresponding to the unlocking of the subaccount's staking ledger on relaychain
		pub unlocking: Vec<UnlockChunk>,
	}

	/// Just a Balance/BlockNumber tuple to encode when a chunk of funds will be unlocked.
	#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
	pub struct UnlockChunk {
		/// Amount of funds to be unlocked.
		#[codec(compact)]
		value: Balance,
		/// Era number at which point it'll be unlocked.
		#[codec(compact)]
		era: EraIndex,
	}

	impl StakingLedger {
		/// Remove entries from `unlocking` that are sufficiently old and the sum of expired
		/// unlocking.
		fn consolidate_unlocked(self, current_era: EraIndex) -> (Self, Balance) {
			let mut expired_unlocking: Balance = Zero::zero();
			let unlocking = self
				.unlocking
				.into_iter()
				.filter(|chunk| {
					if chunk.era > current_era {
						true
					} else {
						expired_unlocking = expired_unlocking.saturating_add(chunk.value);
						false
					}
				})
				.collect();

			(
				Self {
					bonded: self.bonded,
					unlocking,
				},
				expired_unlocking,
			)
		}
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Multi-currency support for asset management
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

		/// Origin represented Governance
		type GovernanceOrigin: EnsureOrigin<<Self as frame_system::Config>::Origin>;

		/// The currency id of the Staking asset
		#[pallet::constant]
		type StakingCurrencyId: Get<CurrencyId>;

		/// The currency id of the Liquid asset
		#[pallet::constant]
		type LiquidCurrencyId: Get<CurrencyId>;

		/// The homa's module id.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// The default exchange rate for liquid currency to staking currency.
		#[pallet::constant]
		type DefaultExchangeRate: Get<ExchangeRate>;

		/// Vault reward of Homa protocol
		#[pallet::constant]
		type TreasuryAccount: Get<Self::AccountId>;

		/// The index list of active Homa subaccounts.
		/// `active` means these subaccounts can continue do bond/unbond operations by Homa.
		#[pallet::constant]
		type ActiveSubAccountsIndexList: Get<Vec<u16>>;

		/// Number of eras for unbonding is expired on relaychain.
		#[pallet::constant]
		type BondingDuration: Get<EraIndex>;

		/// The HomaXcm to manage the staking of sub-account on relaychain.
		type HomaXcm: HomaSubAccountXcm<Self::AccountId, Balance>;
	}

	#[pallet::error]
	pub enum Error<T> {
		///	The mint amount is below the threshold.
		BelowMintThreshold,
		///	The redeem amount to request is below the threshold.
		BelowRedeemThreshold,
		/// The mint will cause staking currency of Homa exceed the soft cap.
		ExceededStakingCurrencySoftCap,
		/// UnclaimedRedemption is not enough, this error is not expected.
		InsufficientUnclaimedRedemption,
		/// Invalid era index to bump, must be greater than RelayChainCurrentEra
		InvalidEraIndex,
		/// Redeem request is not allowed to be fast matched.
		FastMatchIsNotAllowed,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// The minter use staking currency to mint liquid currency. \[minter,
		/// staking_currency_amount, liquid_currency_amount_received\]
		Minted(T::AccountId, Balance, Balance),
		/// Request redeem. \[redeemer, liquid_amount, allow_fast_match\]
		RequestedRedeem(T::AccountId, Balance, bool),
		/// Redeem request has been cancelled. \[redeemer, cancelled_liquid_amount\]
		RedeemRequestCancelled(T::AccountId, Balance),
		/// Redeem request is redeemed partially or fully by fast match. \[redeemer,
		/// matched_liquid_amount, fee_in_liquid, redeemed_staking_amount\]
		RedeemedByFastMatch(T::AccountId, Balance, Balance, Balance),
		/// Redeem request is redeemed by unbond on relaychain. \[redeemer,
		/// era_index_when_unbond, liquid_amount, unbonding_staking_amount\]
		RedeemedByUnbond(T::AccountId, EraIndex, Balance, Balance),
		/// The redeemer withdraw expired redemption. \[redeemer, redeption_amount\]
		WithdrawRedemption(T::AccountId, Balance),
		/// The current era has been bumped. \[new_era_index\]
		CurrentEraBumped(EraIndex),
		/// The bonded amount of subaccount's ledger has been updated. \[sub_account_index,
		/// new_bonded_amount\]
		LedgerBondedUpdated(u16, Balance),
		/// The unlocking of subaccount's ledger has been updated. \[sub_account_index,
		/// new_unlocking\]
		LedgerUnlockingUpdated(u16, Vec<UnlockChunk>),
		/// The soft bonded cap of per sub account has been updated. \[cap_amount\]
		SoftBondedCapPerSubAccountUpdated(Balance),
		/// The estimated reward rate per era of relaychain staking has been updated.
		/// \[reward_rate\]
		EstimatedRewardRatePerEraUpdated(Rate),
		/// The threshold to mint has been updated. \[mint_threshold\]
		MintThresholdUpdated(Balance),
		/// The threshold to redeem has been updated. \[redeem_threshold\]
		RedeemThresholdUpdated(Balance),
		/// The commission rate has been updated. \[commission_rate\]
		CommissionRateUpdated(Rate),
		/// The fast match fee rate has been updated. \[commission_rate\]
		FastMatchFeeRateUpdated(Rate),
	}

	/// The current era of relaychain
	///
	/// RelayChainCurrentEra : EraIndex
	#[pallet::storage]
	#[pallet::getter(fn relay_chain_current_era)]
	pub type RelayChainCurrentEra<T: Config> = StorageValue<_, EraIndex, ValueQuery>;

	// /// The latest processed era of Homa, it should be always <= RelayChainCurrentEra
	// ///
	// /// ProcessedEra : EraIndex
	// #[pallet::storage]
	// #[pallet::getter(fn processed_era)]
	// pub type ProcessedEra<T: Config> = StorageValue<_, EraIndex, ValueQuery>;

	/// The staking ledger of Homa subaccounts.
	///
	/// StakingLedgers map: u16 => Option<StakingLedger>
	#[pallet::storage]
	#[pallet::getter(fn staking_ledgers)]
	pub type StakingLedgers<T: Config> = StorageMap<_, Twox64Concat, u16, StakingLedger, OptionQuery>;

	/// The total staking currency to bond on relaychain when new era,
	/// and that is available to be match fast redeem request.
	/// ToBondPool value: StakingCurrencyAmount
	#[pallet::storage]
	#[pallet::getter(fn to_bond_pool)]
	pub type ToBondPool<T: Config> = StorageValue<_, Balance, ValueQuery>;

	/// The total amount of void liquid currency. It's will not be issued,
	/// used to avoid newly issued LDOT to obtain the incoming staking income from relaychain.
	/// And it is guaranteed that the current exchange rate between liquid currency and staking
	/// currency will not change. It will be reset to 0 at the beginning of the rebalance when new
	/// era.
	///
	/// TotalVoidLiquid value: LiquidCurrencyAmount
	#[pallet::storage]
	#[pallet::getter(fn total_void_liquid)]
	pub type TotalVoidLiquid<T: Config> = StorageValue<_, Balance, ValueQuery>;

	/// The total unclaimed redemption.
	///
	/// UnclaimedRedemption value: StakingCurrencyAmount
	#[pallet::storage]
	#[pallet::getter(fn unclaimed_redemption)]
	pub type UnclaimedRedemption<T: Config> = StorageValue<_, Balance, ValueQuery>;

	/// Requests to redeem staked currencies.
	///
	/// RedeemRequests: Map: AccountId => Option<(liquid_amount: Balance, allow_fast_match: bool)>
	#[pallet::storage]
	#[pallet::getter(fn redeem_requests)]
	pub type RedeemRequests<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, (Balance, bool), OptionQuery>;

	/// The records of unbonding by AccountId.
	///
	/// Unbondings: double_map AccountId, ExpireEraIndex => UnboundingStakingCurrencyAmount
	#[pallet::storage]
	#[pallet::getter(fn unbondings)]
	pub type Unbondings<T: Config> =
		StorageDoubleMap<_, Twox64Concat, T::AccountId, Twox64Concat, EraIndex, Balance, ValueQuery>;

	/// The estimated staking reward rate per era on relaychain.
	///
	/// EstimatedRewardRatePerEra: value: Rate
	#[pallet::storage]
	#[pallet::getter(fn estimated_reward_rate_per_era)]
	pub type EstimatedRewardRatePerEra<T: Config> = StorageValue<_, Rate, ValueQuery>;

	/// Th maximum amount of bonded staking currency for a single sub on relaychain to obtain the
	/// best staking rewards.
	///
	/// SoftBondedCapPerSubAccount: value: Balance
	#[pallet::storage]
	#[pallet::getter(fn soft_bonded_cap_per_sub_account)]
	pub type SoftBondedCapPerSubAccount<T: Config> = StorageValue<_, Balance, ValueQuery>;

	/// Th staking amount of threshold to mint.
	///
	/// MintThreshold: value: Balance
	#[pallet::storage]
	#[pallet::getter(fn mint_threshold)]
	pub type MintThreshold<T: Config> = StorageValue<_, Balance, ValueQuery>;

	/// Th liquid amount of threshold to redeem.
	///
	/// RedeemThreshold: value: Balance
	#[pallet::storage]
	#[pallet::getter(fn redeem_threshold)]
	pub type RedeemThreshold<T: Config> = StorageValue<_, Balance, ValueQuery>;

	/// The rate of Homa drawn from the staking reward as commision.
	/// The draw will be transfer to TreasuryAccount of Homa in liquid currency.
	///
	/// CommissionRate: value: Rate
	#[pallet::storage]
	#[pallet::getter(fn commission_rate)]
	pub type CommissionRate<T: Config> = StorageValue<_, Rate, ValueQuery>;

	/// The fixed fee rate for redeem request is fast matched.
	///
	/// FastMatchFeeRate: value: Rate
	#[pallet::storage]
	#[pallet::getter(fn fast_match_fee_rate)]
	pub type FastMatchFeeRate<T: Config> = StorageValue<_, Rate, ValueQuery>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Mint liquid currency by put locking up amount of staking currency.
		///
		/// Parameters:
		/// - `amount`: The amount of staking currency used to mint liquid currency.
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn mint(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResult {
			let minter = ensure_signed(origin)?;

			// Ensure the amount is above the mint threshold.
			ensure!(amount >= Self::mint_threshold(), Error::<T>::BelowMintThreshold);

			// Ensure the total staking currency will not exceed soft cap.
			ensure!(
				Self::get_total_staking_currency().saturating_add(amount) <= Self::get_staking_currency_soft_cap(),
				Error::<T>::ExceededStakingCurrencySoftCap
			);

			T::Currency::transfer(T::StakingCurrencyId::get(), &minter, &Self::account_id(), amount)?;

			// calculate the liquid amount by the current exchange rate.
			let liquid_amount = Self::convert_staking_to_liquid(amount)?;
			let liquid_issue_to_minter = Rate::one()
				.saturating_add(Self::estimated_reward_rate_per_era())
				.reciprocal()
				.expect("shouldn't be invalid!")
				.saturating_mul_int(liquid_amount);
			let liquid_add_to_void = liquid_amount.saturating_sub(liquid_issue_to_minter);

			T::Currency::deposit(T::LiquidCurrencyId::get(), &minter, liquid_issue_to_minter)?;
			ToBondPool::<T>::mutate(|pool| *pool = pool.saturating_add(amount));
			TotalVoidLiquid::<T>::mutate(|total| *total = total.saturating_add(liquid_add_to_void));

			Self::deposit_event(Event::<T>::Minted(minter, amount, liquid_issue_to_minter));
			Ok(())
		}

		/// Build/Cancel/Overwrite a redeem request, use liquid currency to redeem staking currency.
		/// The redeem request will be executed in two ways:
		/// 1. Redeem by fast match: Homa use staking currency in ToBondPool to match redeem request
		/// in the current era, setting a higher fee_rate can increase the possibility of being fast
		/// matched. 2. Redeem by unbond on relaychain: if redeem request has not been fast matched
		/// in current era, Homa will unbond staking currency on relaychain when the next era
		/// bumped. So redeemer at least wait for the unbonding period + extra 1 era to get the
		/// redemption.
		///
		/// Parameters:
		/// - `amount`: The amount of liquid currency to be requested  redeemed into Staking
		///   currency.
		/// - `allow_fast_match`: allow the request to be fast matched, fast match will take a fixed
		///   rate as fee.
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn request_redeem(
			origin: OriginFor<T>,
			#[pallet::compact] amount: Balance,
			allow_fast_match: bool,
		) -> DispatchResult {
			let redeemer = ensure_signed(origin)?;

			RedeemRequests::<T>::try_mutate_exists(&redeemer, |maybe_request| -> DispatchResult {
				let (previous_request_amount, _) = maybe_request.take().unwrap_or_default();
				let liquid_currency_id = T::LiquidCurrencyId::get();

				ensure!(
					(!previous_request_amount.is_zero() && amount.is_zero()) || amount >= Self::redeem_threshold(),
					Error::<T>::BelowRedeemThreshold
				);

				match amount.cmp(&previous_request_amount) {
					Ordering::Greater =>
					// pay more liquid currency.
					{
						T::Currency::transfer(
							liquid_currency_id,
							&redeemer,
							&Self::account_id(),
							amount.saturating_sub(previous_request_amount),
						)
					}
					Ordering::Less =>
					// refund the difference.
					{
						T::Currency::transfer(
							liquid_currency_id,
							&Self::account_id(),
							&redeemer,
							previous_request_amount.saturating_sub(amount),
						)
					}
					_ => Ok(()),
				}?;

				if !amount.is_zero() {
					*maybe_request = Some((amount, allow_fast_match));
					Self::deposit_event(Event::<T>::RequestedRedeem(redeemer.clone(), amount, allow_fast_match));
				} else if !previous_request_amount.is_zero() {
					Self::deposit_event(Event::<T>::RedeemRequestCancelled(
						redeemer.clone(),
						previous_request_amount,
					));
				}
				Ok(())
			})
		}

		/// Execute fast match for specific redeem requests.
		///
		/// Parameters:
		/// - `redeemer_list`: The list of redeem requests to execute fast redeem.
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn fast_match_redeems(origin: OriginFor<T>, redeemer_list: Vec<T::AccountId>) -> DispatchResult {
			let _ = ensure_signed(origin)?;

			for redeemer in redeemer_list {
				Self::do_fast_match_redeem(&redeemer)?;
			}

			Ok(())
		}

		/// Withdraw the expired redemption of specific redeemer by unbond.
		///
		/// Parameters:
		/// - `redeemer`: redeemer.
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn claim_redemption(origin: OriginFor<T>, redeemer: T::AccountId) -> DispatchResult {
			let _ = ensure_signed(origin)?;

			let mut available_staking: Balance = Zero::zero();
			Unbondings::<T>::iter_prefix(&redeemer)
				.filter(|(era_index, _)| era_index <= &Self::relay_chain_current_era())
				.for_each(|(expired_era_index, unbonded)| {
					available_staking = available_staking.saturating_add(unbonded);
					Unbondings::<T>::remove(&redeemer, expired_era_index);
				});
			UnclaimedRedemption::<T>::try_mutate(|total| -> DispatchResult {
				*total = total
					.checked_sub(available_staking)
					.ok_or(Error::<T>::InsufficientUnclaimedRedemption)?;
				Ok(())
			})?;
			T::Currency::transfer(
				T::StakingCurrencyId::get(),
				&Self::account_id(),
				&redeemer,
				available_staking,
			)?;

			Self::deposit_event(Event::<T>::WithdrawRedemption(redeemer, available_staking));
			Ok(())
		}

		/// Bump the current era to keep consistent with relaychain.
		/// Requires `GovernanceOrigin`
		///
		/// Parameters:
		/// - `new_era`: the latest era index of relaychain.
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn bump_current_era(origin: OriginFor<T>, new_era: EraIndex) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			RelayChainCurrentEra::<T>::try_mutate(|current_era| -> DispatchResult {
				ensure!(new_era > *current_era, Error::<T>::InvalidEraIndex);

				// reset void liquid to zero firstly, to guarantee
				TotalVoidLiquid::<T>::put(0);

				// TODO: consider execute rebalance on on_idle, before the processing is completed,
				// the mint and request_redeem functions should be unavailable.
				// Rebalance:
				Self::draw_staking_reward(new_era, *current_era)?;
				Self::process_scheduled_unbond(new_era)?;
				Self::process_to_bond_pool(new_era)?;
				Self::process_redeem_requests(new_era)?;

				// bump current era to latest.
				*current_era = new_era;

				Self::deposit_event(Event::<T>::CurrentEraBumped(new_era));
				Ok(())
			})
		}

		/// Update the bonded and unbonding to local subaccounts ledger according to the ledger on
		/// relaychain. Requires `GovernanceOrigin`
		///
		/// Parameters:
		/// - `new_era`: the latest era index of relaychain.
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn update_ledgers(
			origin: OriginFor<T>,
			updates: Vec<(u16, Option<Balance>, Option<Vec<UnlockChunk>>)>,
		) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			for (sub_account_index, bonded_change, unlocking_change) in updates {
				Self::do_update_ledger(sub_account_index, |ledger| -> DispatchResult {
					if let Some(bonded) = bonded_change {
						ledger.bonded = bonded;
					}
					if let Some(unlocking) = unlocking_change {
						ledger.unlocking = unlocking;
					}
					Ok(())
				})?;
			}

			Ok(())
		}

		/// Sets the params of Homa.
		/// Requires `GovernanceOrigin`
		///
		/// Parameters:
		/// - `soft_bonded_cap_per_sub_account`:  soft cap of staking amount for a single nominator
		///   on relaychain to obtain the best staking rewards.
		/// - `estimated_reward_rate_per_era`: the esstaking yield of each era on the current relay
		///   chain
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn update_homa_params(
			origin: OriginFor<T>,
			soft_bonded_cap_per_sub_account: Option<Balance>,
			estimated_reward_rate_per_era: Option<Rate>,
			mint_threshold: Option<Balance>,
			redeem_threshold: Option<Balance>,
			commission_rate: Option<Rate>,
			fast_match_fee_rate: Option<Rate>,
		) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			if let Some(cap) = soft_bonded_cap_per_sub_account {
				SoftBondedCapPerSubAccount::<T>::put(cap);
				Self::deposit_event(Event::<T>::SoftBondedCapPerSubAccountUpdated(cap));
			}
			if let Some(rate) = estimated_reward_rate_per_era {
				EstimatedRewardRatePerEra::<T>::put(rate);
				Self::deposit_event(Event::<T>::EstimatedRewardRatePerEraUpdated(rate));
			}
			if let Some(threshold) = mint_threshold {
				MintThreshold::<T>::put(threshold);
				Self::deposit_event(Event::<T>::MintThresholdUpdated(threshold));
			}
			if let Some(threshold) = redeem_threshold {
				RedeemThreshold::<T>::put(threshold);
				Self::deposit_event(Event::<T>::RedeemThresholdUpdated(threshold));
			}
			if let Some(rate) = commission_rate {
				CommissionRate::<T>::put(rate);
				Self::deposit_event(Event::<T>::CommissionRateUpdated(rate));
			}
			if let Some(rate) = fast_match_fee_rate {
				FastMatchFeeRate::<T>::put(rate);
				Self::deposit_event(Event::<T>::FastMatchFeeRateUpdated(rate));
			}

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Module account id
		pub fn account_id() -> T::AccountId {
			T::PalletId::get().into_account()
		}

		fn do_update_ledger<R, E>(
			sub_account_index: u16,
			f: impl FnOnce(&mut StakingLedger) -> sp_std::result::Result<R, E>,
		) -> sp_std::result::Result<R, E> {
			StakingLedgers::<T>::try_mutate_exists(sub_account_index, |maybe_ledger| {
				let mut ledger = maybe_ledger.take().unwrap_or_default();
				let old_ledger = ledger.clone();

				f(&mut ledger).map(move |result| {
					if ledger.bonded != old_ledger.bonded {
						Self::deposit_event(Event::<T>::LedgerBondedUpdated(sub_account_index, ledger.bonded));
					}
					if ledger.unlocking != old_ledger.unlocking {
						Self::deposit_event(Event::<T>::LedgerUnlockingUpdated(
							sub_account_index,
							ledger.unlocking.clone(),
						));
					}

					*maybe_ledger = if ledger == Default::default() {
						None
					} else {
						Some(ledger)
					};

					result
				})
			})
		}

		/// Get the soft cap of total staking currency of Homa.
		/// Soft cap = ActiveSubAccountsIndexList.len() * SoftBondedCapPerSubAccount
		pub fn get_staking_currency_soft_cap() -> Balance {
			Self::soft_bonded_cap_per_sub_account()
				.saturating_mul(T::ActiveSubAccountsIndexList::get().len() as Balance)
		}

		/// Calculate the total amount of bonded staking currency.
		pub fn get_total_bonded() -> Balance {
			StakingLedgers::<T>::iter().fold(Zero::zero(), |total_bonded, (_, ledger)| {
				total_bonded.saturating_add(ledger.bonded)
			})
		}

		/// Calculate the total amount of staking currency belong to Homa.
		pub fn get_total_staking_currency() -> Balance {
			Self::get_total_bonded().saturating_add(Self::to_bond_pool())
		}

		/// Calculate the current exchange rate between the staking currency and liquid currency.
		/// Note: ExchangeRate(staking : liquid) = total_staking_amount / (liquid_total_issuance +
		/// total_void_liquid) If the exchange rate cannot be calculated, T::DefaultExchangeRate is
		/// used.
		pub fn current_exchange_rate() -> ExchangeRate {
			let total_staking = Self::get_total_staking_currency();
			let total_liquid =
				T::Currency::total_issuance(T::LiquidCurrencyId::get()).saturating_add(Self::total_void_liquid());
			if total_staking.is_zero() {
				T::DefaultExchangeRate::get()
			} else {
				ExchangeRate::checked_from_rational(total_staking, total_liquid)
					.unwrap_or_else(T::DefaultExchangeRate::get)
			}
		}

		/// Calculate the amount of staking currency converted from liquid currency by current
		/// exchange rate.
		pub fn convert_liquid_to_staking(liquid_amount: Balance) -> Result<Balance, DispatchError> {
			Self::current_exchange_rate()
				.checked_mul_int(liquid_amount)
				.ok_or(DispatchError::Arithmetic(ArithmeticError::Overflow))
		}

		/// Calculate the amount of liquid currency converted from staking currency by current
		/// exchange rate.
		pub fn convert_staking_to_liquid(staking_amount: Balance) -> Result<Balance, DispatchError> {
			Self::current_exchange_rate()
				.reciprocal()
				.unwrap_or_else(|| T::DefaultExchangeRate::get().reciprocal().unwrap())
				.checked_mul_int(staking_amount)
				.ok_or(DispatchError::Arithmetic(ArithmeticError::Overflow))
		}

		#[transactional]
		pub fn do_fast_match_redeem(redeemer: &T::AccountId) -> DispatchResult {
			RedeemRequests::<T>::try_mutate_exists(redeemer, |maybe_request| -> DispatchResult {
				if let Some((request_amount, allow_fast_match)) = maybe_request.take() {
					ensure!(allow_fast_match, Error::<T>::FastMatchIsNotAllowed);

					// calculate the liquid currency limit can be used to redeem based on ToBondPool at fee_rate.
					let available_staking_currency = Self::to_bond_pool();
					let liquid_currency_limit = Self::convert_staking_to_liquid(available_staking_currency)?;
					let fast_match_fee_rate = Self::fast_match_fee_rate();
					let liquid_limit_at_fee_rate = Rate::one()
						.saturating_sub(fast_match_fee_rate)
						.reciprocal()
						.unwrap_or_else(Bounded::max_value)
						.saturating_mul_int(liquid_currency_limit);
					let module_account = Self::account_id();

					// calculate the acutal liquid currency to be used to redeem
					let actual_liquid_to_redeem = if liquid_limit_at_fee_rate >= request_amount {
						request_amount
					} else {
						// if cannot fast match the request amount fully, at least keep RedeemThreshold as remainer.
						liquid_limit_at_fee_rate.min(request_amount.saturating_sub(Self::redeem_threshold()))
					};

					if !actual_liquid_to_redeem.is_zero() {
						let liquid_to_burn = Rate::one()
							.saturating_sub(fast_match_fee_rate)
							.saturating_mul_int(actual_liquid_to_redeem);
						let redeemed_staking = Self::convert_liquid_to_staking(liquid_to_burn)?;
						let fee_in_liquid = actual_liquid_to_redeem.saturating_sub(liquid_to_burn);

						// burn liquid_to_burn for redeemed_staking and burn fee_in_liquid to reward all holders of
						// liquid currency.
						T::Currency::withdraw(T::LiquidCurrencyId::get(), &module_account, actual_liquid_to_redeem)?;

						// transfer redeemed_staking to redeemer.
						T::Currency::transfer(
							T::StakingCurrencyId::get(),
							&module_account,
							redeemer,
							redeemed_staking,
						)?;
						ToBondPool::<T>::mutate(|pool| *pool = pool.saturating_sub(redeemed_staking));

						Self::deposit_event(Event::<T>::RedeemedByFastMatch(
							redeemer.clone(),
							actual_liquid_to_redeem,
							fee_in_liquid,
							redeemed_staking,
						));
					}

					// update request amount
					let remainer_request_amount = request_amount.saturating_sub(actual_liquid_to_redeem);
					if !remainer_request_amount.is_zero() {
						*maybe_request = Some((remainer_request_amount, allow_fast_match));
					}
				}

				Ok(())
			})
		}

		/// Draw commission to TreasuryAccount from estimated staking rewards. Commission will be
		/// given to TreasuryAccount by issuing liquid currency. Note: This will cause some losses
		/// to the minters in previous_era, because they have been already deducted some liquid
		/// currency amount when mint in previous_era. Until there is a better way to calculate,
		/// this part of the loss can only be regarded as an implicit mint fee!
		#[transactional]
		pub fn draw_staking_reward(new_era: EraIndex, previous_era: EraIndex) -> DispatchResult {
			let era_interval = new_era.saturating_sub(previous_era);
			let liquid_currency_id = T::LiquidCurrencyId::get();
			let bond_ratio = Ratio::checked_from_rational(Self::get_total_bonded(), Self::get_total_staking_currency())
				.unwrap_or_else(Ratio::zero);
			let draw_rate = bond_ratio
				.saturating_mul(Self::estimated_reward_rate_per_era())
				.saturating_add(Rate::one())
				.saturating_pow(era_interval.unique_saturated_into())
				.saturating_sub(Rate::one())
				.saturating_mul(Self::commission_rate());
			let inflation_rate = Rate::one()
				.saturating_add(draw_rate)
				.reciprocal()
				.expect("shouldn't be invalid!");

			let liquid_amount_as_commision =
				inflation_rate.saturating_mul_int(T::Currency::total_issuance(liquid_currency_id));
			T::Currency::deposit(
				liquid_currency_id,
				&T::TreasuryAccount::get(),
				liquid_amount_as_commision,
			)
		}

		/// Get back unbonded of all subaccounts on relaychain by XCM.
		/// The staking currency withdrew becomes available to be redeemed.
		#[transactional]
		pub fn process_scheduled_unbond(new_era: EraIndex) -> DispatchResult {
			log::debug!(
				target: "homa",
				"process scheduled unbond on era: {:?}",
				new_era
			);

			let mut total_withdrawn_staking: Balance = Zero::zero();

			// iterate all subaccounts
			for (sub_account_index, ledger) in StakingLedgers::<T>::iter() {
				let (new_ledger, expired_unlocking) = ledger.consolidate_unlocked(new_era);

				if !expired_unlocking.is_zero() {
					T::HomaXcm::withdraw_unbonded_from_sub_account(sub_account_index, expired_unlocking)?;

					// udpate ledger
					Self::do_update_ledger(sub_account_index, |before| -> DispatchResult {
						*before = new_ledger;
						Ok(())
					})?;
					total_withdrawn_staking = total_withdrawn_staking.saturating_add(expired_unlocking);
				}
			}

			// issue withdrawn unbonded to module account for redeemer to claim
			T::Currency::deposit(
				T::StakingCurrencyId::get(),
				&Self::account_id(),
				total_withdrawn_staking,
			)?;
			UnclaimedRedemption::<T>::mutate(|total| *total = total.saturating_add(total_withdrawn_staking));

			Ok(())
		}

		/// Distribute PoolToBond to ActiveSubAccountsIndexList, then cross-transfer the
		/// distribution amount to the subaccounts on relaychain and bond it by XCM.
		#[transactional]
		pub fn process_to_bond_pool(new_era: EraIndex) -> DispatchResult {
			log::debug!(
				target: "homa",
				"process to bond pool on era: {:?}",
				new_era
			);

			let xcm_transfer_fee = T::HomaXcm::get_xcm_transfer_fee();
			let bonded_list: Vec<(u16, Balance)> = T::ActiveSubAccountsIndexList::get()
				.iter()
				.map(|index| (*index, Self::staking_ledgers(index).unwrap_or_default().bonded))
				.collect();
			let (distribution, remainer) = distribute_increment::<u16>(
				bonded_list,
				Self::to_bond_pool(),
				Some(Self::soft_bonded_cap_per_sub_account()),
				Some(xcm_transfer_fee),
			);

			// subaccounts execute the distribution
			for (sub_account_index, amount) in distribution {
				if !amount.is_zero() {
					T::HomaXcm::transfer_staking_to_sub_account(&Self::account_id(), sub_account_index, amount)?;

					let bond_amount = amount.saturating_sub(xcm_transfer_fee);
					T::HomaXcm::bond_extra_on_sub_account(sub_account_index, bond_amount)?;

					// udpate ledger
					Self::do_update_ledger(sub_account_index, |ledger| -> DispatchResult {
						ledger.bonded = ledger.bonded.saturating_add(bond_amount);
						Ok(())
					})?;
				}
			}

			// update pool
			ToBondPool::<T>::mutate(|pool| *pool = remainer);
			Ok(())
		}

		/// Process redeem requests and subaccounts do unbond on relaychain by XCM message.
		#[transactional]
		pub fn process_redeem_requests(new_era: EraIndex) -> DispatchResult {
			log::debug!(
				target: "homa",
				"process redeem requests on era: {:?}",
				new_era
			);

			let mut total_redeem_amount: Balance = Zero::zero();
			let era_index_to_expire = new_era + T::BondingDuration::get();

			// drain RedeemRequests and insert to Unbondings
			for (redeemer, (redeem_amount, _)) in RedeemRequests::<T>::drain() {
				total_redeem_amount = total_redeem_amount.saturating_add(redeem_amount);
				let redemption_amount = Self::convert_liquid_to_staking(redeem_amount)?;
				Unbondings::<T>::insert(&redeemer, era_index_to_expire, redemption_amount);
				Self::deposit_event(Event::<T>::RedeemedByUnbond(
					redeemer,
					new_era,
					redeem_amount,
					redemption_amount,
				));
			}

			// calculate the distribution for unbond
			let staking_amount_to_unbond = Self::convert_liquid_to_staking(total_redeem_amount)?;
			let bonded_list: Vec<(u16, Balance)> = T::ActiveSubAccountsIndexList::get()
				.iter()
				.map(|index| (*index, Self::staking_ledgers(index).unwrap_or_default().bonded))
				.collect();
			let (distribution, _) = distribute_decrement::<u16>(bonded_list, staking_amount_to_unbond, None, None);

			// subaccounts execute the distribution
			for (sub_account_index, unbond_amount) in distribution {
				if !unbond_amount.is_zero() {
					T::HomaXcm::unbond_on_sub_account(sub_account_index, unbond_amount)?;

					// udpate ledger
					Self::do_update_ledger(sub_account_index, |ledger| -> DispatchResult {
						ledger.bonded = ledger.bonded.saturating_sub(unbond_amount);
						ledger.unlocking.push(UnlockChunk {
							value: unbond_amount,
							era: era_index_to_expire,
						});
						Ok(())
					})?;
				}
			}

			// burn total_redeem_amount.
			T::Currency::withdraw(T::LiquidCurrencyId::get(), &Self::account_id(), total_redeem_amount)
		}
	}

	impl<T: Config> ExchangeRateProvider for Pallet<T> {
		fn get_exchange_rate() -> ExchangeRate {
			Self::current_exchange_rate()
		}
	}
}

/// Helpers for distribute increment/decrement to as possible to keep the list balanced after
/// distribution.
pub fn distribute_increment<Index>(
	mut amount_list: Vec<(Index, Balance)>,
	total_increment: Balance,
	amount_cap: Option<Balance>,
	minimum_increment: Option<Balance>,
) -> (Vec<(Index, Balance)>, Balance) {
	let mut remain_increment = total_increment;
	let mut distribution_list: Vec<(Index, Balance)> = vec![];

	// Sort by amount in ascending order
	amount_list.sort_by(|a, b| a.1.cmp(&b.1));

	for (index, amount) in amount_list {
		if remain_increment.is_zero() || remain_increment < minimum_increment.unwrap_or_else(Bounded::max_value) {
			break;
		}

		let increment_distribution = amount_cap
			.unwrap_or_else(Bounded::max_value)
			.saturating_add(amount)
			.min(remain_increment);
		if increment_distribution.is_zero()
			|| increment_distribution < minimum_increment.unwrap_or_else(Bounded::max_value)
		{
			continue;
		}
		distribution_list.push((index, increment_distribution));
		remain_increment = remain_increment.saturating_sub(increment_distribution);
	}

	(distribution_list, remain_increment)
}

pub fn distribute_decrement<Index>(
	mut amount_list: Vec<(Index, Balance)>,
	total_decrement: Balance,
	amount_remainer: Option<Balance>,
	minimum_decrement: Option<Balance>,
) -> (Vec<(Index, Balance)>, Balance) {
	let mut remain_decrement = total_decrement;
	let mut distribution_list: Vec<(Index, Balance)> = vec![];

	// Sort by amount in descending order
	amount_list.sort_by(|a, b| b.1.cmp(&a.1));

	for (index, amount) in amount_list {
		if remain_decrement.is_zero() || remain_decrement < minimum_decrement.unwrap_or_else(Bounded::max_value) {
			break;
		}

		let decrement_distribution = amount
			.saturating_sub(amount_remainer.unwrap_or_else(Bounded::min_value))
			.min(remain_decrement);
		if decrement_distribution.is_zero()
			|| decrement_distribution < minimum_decrement.unwrap_or_else(Bounded::min_value)
		{
			continue;
		}
		distribution_list.push((index, decrement_distribution));
		remain_decrement = remain_decrement.saturating_sub(decrement_distribution);
	}

	(distribution_list, remain_decrement)
}
