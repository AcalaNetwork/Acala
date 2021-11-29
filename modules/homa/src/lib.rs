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

use frame_support::{log, pallet_prelude::*, transactional, weights::Weight, BoundedVec, PalletId};
use frame_system::{ensure_signed, pallet_prelude::*};
use module_support::{CallBuilder, ExchangeRate, ExchangeRateProvider, Rate};
use orml_traits::{arithmetic::Signed, BalanceStatus, MultiCurrency, MultiCurrencyExtended, XcmTransfer};
use pallet_staking::EraIndex;
use primitives::{Balance, CurrencyId};
use scale_info::TypeInfo;
use sp_arithmetic::traits::CheckedRem;
use sp_runtime::{
	traits::{AccountIdConversion, BlockNumberProvider, Bounded, Convert, One, Saturating, Zero},
	ArithmeticError, FixedPointNumber, Permill,
};
use sp_std::{
	cmp::{min, Ordering},
	convert::{From, TryFrom, TryInto},
	ops::Mul,
	prelude::*,
	vec,
	vec::Vec,
};
use xcm::latest::prelude::*;

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

	pub type SubAccountIndex = u16;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_xcm::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Multi-currency support for asset management
		type Currency: MultiCurrencyExtended<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

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

		/// The threshold for mint operation in staking currency.
		#[pallet::constant]
		type MintThreshold: Get<Balance>;

		/// The threshold for redeem operation in liquid currency.
		#[pallet::constant]
		type RedeemThreshold: Get<Balance>;

		/// The account of parachain on the relaychain.
		#[pallet::constant]
		type ParachainAccount: Get<Self::AccountId>;

		/// The index list of active Homa subaccounts.
		/// `active` means these subaccounts can continue do bond/unbond operations by Homa.
		#[pallet::constant]
		type ActiveSubAccountsIndexList: Get<Vec<SubAccountIndex>>;

		/// The bonded soft cap for each subaccount, use len(ActiveSubAccountsIndexList) *
		/// SoftBondedCapPerSubAccount as the staking currency cap.
		#[pallet::constant]
		type SoftBondedCapPerSubAccount: Get<Balance>;

		/// The keepers list which are allowed to do fast match redeem request.
		#[pallet::constant]
		type FastMatchKeepers: Get<Vec<Self::AccountId>>;

		/// Number of eras for unbonding is expired on relaychain.
		#[pallet::constant]
		type BondingDuration: Get<EraIndex>;

		/// Unbonding slashing spans for unbonding on the relaychain.
		#[pallet::constant]
		type RelayChainUnbondingSlashingSpans: Get<EraIndex>;

		/// The estimated staking reward rate per era on relaychain.
		#[pallet::constant]
		type EstimatedRewardRatePerEra: Get<Rate>;

		/// The fixed staking currency cost of transaction fee for XCMTransfer.
		#[pallet::constant]
		type XcmTransferFee: Get<Balance>;

		/// The fixed staking currency cost of extra fee for xcm message
		#[pallet::constant]
		type XcmMessageFee: Get<Balance>;

		/// The Call builder for communicating with RelayChain via XCM messaging.
		type RelayChainCallBuilder: CallBuilder<AccountId = Self::AccountId, Balance = Balance>;

		/// The interface to Cross-chain transfer.
		type XcmTransfer: XcmTransfer<Self::AccountId, Balance, CurrencyId>;

		/// The convert for convert sovereign subacocunt index to the MultiLocation where the
		/// staking currencies are sent to.
		type SovereignSubAccountLocationConvert: Convert<SubAccountIndex, MultiLocation>;
	}

	#[pallet::error]
	pub enum Error<T> {
		///	The mint amount is below the threshold.
		BelowMintThreshold,
		///	The redeem amount to request is below the threshold.
		BelowRedeemThreshold,
		/// The caller is not in `FastMatchKeepers` list.
		NotAllowedKeeper,
		/// The mint will cause staking currency of Homa exceed the soft cap.
		ExceededStakingCurrencySoftCap,
		/// UnclaimedRedemption is not enough, this error is not expected.
		InsufficientUnclaimedRedemption,
		/// Invalid era index to bump, must be greater than RelayChainCurrentEra
		InvalidEraIndex,
		/// The xcm operation have failed
		XcmFailed,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// The minter use staking currency to mint liquid currency. \[minter,
		/// staking_currency_amount, liquid_currency_amount_received\]
		Minted(T::AccountId, Balance, Balance),
		/// Request redeem. \[redeemer, liquid_amount, fast_match_fee_rate\]
		RequestedRedeem(T::AccountId, Balance, Rate),
		/// Redeem request has been cancelled. \[redeemer, cancelled_liquid_amount\]
		RedeemRequestCancelled(T::AccountId, Balance),
		/// Redeem request is redeemed partially or fully by fast match. \[redeemer,
		/// matched_liquid_amount, fee_in_liquid, redeemed_staking_amount\]
		RedeemedByFastMatch(T::AccountId, Balance, Balance, Balance),
		/// The redeemer withdraw expired redemption. \[redeemer, redeption_amount\]
		WithdrawRedemption(T::AccountId, Balance),
		/// The redeemer withdraw expired redemption. \[redeemer, redeption_amount\]
		XcmDestWeightUpdated(Weight),
		/// The current era has been bumped. \[new_era_index\]
		CurrentEraBumped(EraIndex),
		/// The bonded amount of subaccount's ledger has been updated. \[sub_account_index,
		/// new_bonded_amount\]
		LedgerBondedUpdated(SubAccountIndex, Balance),
		/// The unlocking of subaccount's ledger has been updated. \[sub_account_index,
		/// new_unlocking\]
		LedgerUnlockingUpdated(SubAccountIndex, Vec<UnlockChunk>),
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
	/// StakingLedgers map: SubAccountIndex => Option<StakingLedger>
	#[pallet::storage]
	#[pallet::getter(fn staking_ledgers)]
	pub type StakingLedgers<T: Config> = StorageMap<_, Twox64Concat, SubAccountIndex, StakingLedger, OptionQuery>;

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
	/// RedeemRequests: Map: AccountId => Option<(liquid_amount: Balance, addtional_fee: Rate)>
	#[pallet::storage]
	#[pallet::getter(fn redeem_requests)]
	pub type RedeemRequests<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, (Balance, Rate), OptionQuery>;

	/// The records of unbonding by AccountId.
	///
	/// Unbondings: double_map AccountId, ExpireEraIndex => UnboundingStakingCurrencyAmount
	#[pallet::storage]
	#[pallet::getter(fn unbondings)]
	pub type Unbondings<T: Config> =
		StorageDoubleMap<_, Twox64Concat, T::AccountId, Twox64Concat, EraIndex, Balance, ValueQuery>;

	/// The weight limit for excution XCM msg on relaychain. Must be greater than the weight of
	/// the XCM msg that sended by Homa, otherwise the execution of XCM msg will fail.
	/// Consider all possible xcm msgs sended by Homa, and use the maximum as the limit.
	///
	/// xcm_dest_weight: value: Weight
	#[pallet::storage]
	#[pallet::getter(fn xcm_dest_weight)]
	pub type XcmDestWeight<T: Config> = StorageValue<_, Weight, ValueQuery>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn integrity_test() {
			assert!(!T::DefaultExchangeRate::get().is_zero());
			assert!(T::MintThreshold::get() >= T::XcmTransferFee::get());
		}
	}

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
			ensure!(amount > T::MintThreshold::get(), Error::<T>::BelowMintThreshold);

			// Ensure the total staking currency will not exceed soft cap.
			ensure!(
				Self::get_total_staking_currency().saturating_add(amount) <= Self::get_staking_currency_soft_cap(),
				Error::<T>::ExceededStakingCurrencySoftCap
			);

			T::Currency::transfer(T::StakingCurrencyId::get(), &minter, &Self::account_id(), amount)?;

			// calculate the liquid amount by the current exchange rate.
			let liquid_amount = Self::convert_staking_to_liquid(amount)?;
			let liquid_issue_to_minter = Rate::one()
				.saturating_add(T::EstimatedRewardRatePerEra::get())
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
		/// - `fast_match_fee_rate`: Fee rate for pay fast match.
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn request_redeem(
			origin: OriginFor<T>,
			#[pallet::compact] amount: Balance,
			fast_match_fee_rate: Rate,
		) -> DispatchResult {
			let redeemer = ensure_signed(origin)?;

			RedeemRequests::<T>::try_mutate_exists(&redeemer, |maybe_request| -> DispatchResult {
				let (previous_request_amount, _) = maybe_request.take().unwrap_or_default();
				let liquid_currency_id = T::LiquidCurrencyId::get();

				ensure!(
					(!previous_request_amount.is_zero() && amount.is_zero()) || amount >= T::RedeemThreshold::get(),
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
					*maybe_request = Some((amount, fast_match_fee_rate));
					Self::deposit_event(Event::<T>::RequestedRedeem(
						redeemer.clone(),
						amount,
						fast_match_fee_rate,
					));
				} else {
					if !previous_request_amount.is_zero() {
						Self::deposit_event(Event::<T>::RedeemRequestCancelled(
							redeemer.clone(),
							previous_request_amount,
						));
					}
				}
				Ok(())
			})
		}

		/// Execute fast match for specific redeem requests.
		/// Caller must be in `FastMatchKeepers` list.
		///
		/// Parameters:
		/// - `redeemer_list`: The list of redeem requests to execute fast redeem.
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn fast_match_redeems(origin: OriginFor<T>, redeemer_list: Vec<T::AccountId>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(T::FastMatchKeepers::get().contains(&who), Error::<T>::NotAllowedKeeper);

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
					available_staking = available_staking.saturating_add(available_staking);
					Unbondings::<T>::remove(&redeemer, expired_era_index);
				});
			UnclaimedRedemption::<T>::try_mutate(|total| -> DispatchResult {
				*total = total
					.checked_sub(available_staking)
					.ok_or(Error::<T>::InsufficientUnclaimedRedemption)?;
				Ok(())
			});
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
				*current_era = new_era;

				// reset void liquid to zero firstly, to guarantee
				TotalVoidLiquid::<T>::put(0);

				// TODO: consider execute rebalance on on_idle, before the processing is completed,
				// the mint and request_redeem functions should be unavailable.
				// Rebalance:
				Self::process_scheduled_unbond(new_era)?;
				Self::process_to_bond_pool(new_era)?;
				Self::process_redeem_requests(new_era)?;

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
			updates: Vec<(SubAccountIndex, Option<Balance>, Option<Vec<UnlockChunk>>)>,
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

		/// Sets the xcm_dest_weight for XCM staking operations.
		/// Requires `GovernanceOrigin`
		///
		/// Parameters:
		/// - `xcm_dest_weight`: The new weight for XCM staking operations.
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn update_xcm_dest_weight(
			origin: OriginFor<T>,
			#[pallet::compact] xcm_dest_weight: Weight,
		) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			XcmDestWeight::<T>::put(xcm_dest_weight);
			Self::deposit_event(Event::<T>::XcmDestWeightUpdated(xcm_dest_weight));
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Module account id
		pub fn account_id() -> T::AccountId {
			T::PalletId::get().into_account()
		}

		fn do_update_ledger<R, E>(
			sub_account_index: SubAccountIndex,
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
			T::SoftBondedCapPerSubAccount::get().saturating_mul(T::ActiveSubAccountsIndexList::get().len() as Balance)
		}

		/// Calculate the total amount of staking currency belong to Homa.
		pub fn get_total_staking_currency() -> Balance {
			StakingLedgers::<T>::iter().fold(Self::to_bond_pool(), |total_bonded, (_, ledger)| {
				total_bonded.saturating_add(ledger.bonded)
			})
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
				if let Some((request_amount, fee_rate)) = maybe_request.take() {
					// calculate the liquid currency limit can be used to redeem based on ToBondPool at fee_rate.
					let available_staking_currency = Self::to_bond_pool();
					let liquid_currency_limit = Self::convert_staking_to_liquid(available_staking_currency)?;
					let liquid_limit_at_fee_rate = Rate::one()
						.saturating_sub(fee_rate)
						.reciprocal()
						.unwrap_or_else(Bounded::max_value)
						.saturating_mul_int(liquid_currency_limit);
					let module_account = Self::account_id();

					// calculate the acutal liquid currency to be used to redeem
					let actual_liquid_to_redeem = if liquid_limit_at_fee_rate >= request_amount {
						request_amount
					} else {
						// if cannot fast match the request amount fully, at least keep RedeemThreshold as remainer.
						liquid_limit_at_fee_rate.min(request_amount.saturating_sub(T::RedeemThreshold::get()))
					};

					if !actual_liquid_to_redeem.is_zero() {
						let liquid_to_burn = Rate::one()
							.saturating_sub(fee_rate)
							.saturating_mul_int(actual_liquid_to_redeem);
						let redeemed_staking = Self::convert_liquid_to_staking(liquid_to_burn)?;
						let fee_in_liquid = actual_liquid_to_redeem.saturating_sub(liquid_to_burn);

						// TODO: record the fee_in_liquid reward it to HomaTreasury as benifit.

						// burn liquid_to_burn
						T::Currency::withdraw(T::LiquidCurrencyId::get(), &module_account, liquid_to_burn)?;

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
						*maybe_request = Some((remainer_request_amount, fee_rate));
					}
				}

				Ok(())
			})
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
					Self::withdraw_unbonded_from_relaychain(sub_account_index, expired_unlocking)?;

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

			let xcm_transfer_fee = T::XcmTransferFee::get();
			let bonded_list: Vec<(SubAccountIndex, Balance)> = T::ActiveSubAccountsIndexList::get()
				.iter()
				.map(|index| (*index, Self::staking_ledgers(index).unwrap_or_default().bonded))
				.collect();
			let (distribution, remainer) = distribute_increment::<SubAccountIndex>(
				bonded_list,
				Self::to_bond_pool(),
				Some(T::SoftBondedCapPerSubAccount::get()),
				Some(xcm_transfer_fee),
			);

			// subaccounts execute the distribution
			for (sub_account_index, amount) in distribution {
				if !amount.is_zero() {
					Self::transfer_and_bond_to_relaychain(sub_account_index, amount)?;

					// udpate ledger
					Self::do_update_ledger(sub_account_index, |ledger| -> DispatchResult {
						ledger.bonded = ledger.bonded.saturating_add(amount.saturating_sub(xcm_transfer_fee));
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
				Unbondings::<T>::insert(redeemer, era_index_to_expire, redemption_amount);
			}

			// calculate the distribution for unbond
			let staking_amount_to_unbond = Self::convert_liquid_to_staking(total_redeem_amount)?;
			let bonded_list: Vec<(SubAccountIndex, Balance)> = T::ActiveSubAccountsIndexList::get()
				.iter()
				.map(|index| (*index, Self::staking_ledgers(index).unwrap_or_default().bonded))
				.collect();
			let (distribution, _) =
				distribute_decrement::<SubAccountIndex>(bonded_list, staking_amount_to_unbond, None, None);

			// subaccounts execute the distribution
			for (sub_account_index, unbond_amount) in distribution {
				if !unbond_amount.is_zero() {
					Self::unbond_on_relaychain(sub_account_index, unbond_amount)?;

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
			T::Currency::withdraw(T::LiquidCurrencyId::get(), &Self::account_id(), total_redeem_amount)?;

			Ok(())
		}

		/// Send XCM message to the relaychain to withdraw_unbonded staking currency from
		/// subaccount.
		pub fn withdraw_unbonded_from_relaychain(
			sub_account_index: SubAccountIndex,
			amount: Balance,
		) -> DispatchResult {
			let xcm_message = T::RelayChainCallBuilder::finalize_call_into_xcm_message(
				T::RelayChainCallBuilder::utility_as_derivative_call(
					T::RelayChainCallBuilder::utility_batch_call(vec![
						T::RelayChainCallBuilder::staking_withdraw_unbonded(T::RelayChainUnbondingSlashingSpans::get()),
						T::RelayChainCallBuilder::balances_transfer_keep_alive(T::ParachainAccount::get(), amount),
					]),
					sub_account_index,
				),
				T::XcmMessageFee::get(),
				Self::xcm_dest_weight(),
			);

			let result = pallet_xcm::Pallet::<T>::send_xcm(Here, Parent, xcm_message);
			log::debug!(
				target: "homa",
				"subaccount {:?} send XCM to withdraw unbonded {:?} on relaychain result: {:?}",
				sub_account_index, amount, result
			);
			ensure!(result.is_ok(), Error::<T>::XcmFailed);
			Ok(())
		}

		/// Cross-chain transfer staking currency to subaccount and send XCM message to the
		/// relaychain to bond it.
		pub fn transfer_and_bond_to_relaychain(sub_account_index: SubAccountIndex, amount: Balance) -> DispatchResult {
			T::XcmTransfer::transfer(
				Self::account_id(),
				T::StakingCurrencyId::get(),
				amount,
				T::SovereignSubAccountLocationConvert::convert(sub_account_index),
				Self::xcm_dest_weight(),
			)?;

			// subaccount will pay the XcmTransferFee, so the actual staking amount received should deduct it.
			let bond_amount = amount.saturating_sub(T::XcmTransferFee::get());
			let xcm_message = T::RelayChainCallBuilder::finalize_call_into_xcm_message(
				T::RelayChainCallBuilder::utility_as_derivative_call(
					T::RelayChainCallBuilder::staking_bond_extra(bond_amount),
					sub_account_index,
				),
				T::XcmMessageFee::get(),
				Self::xcm_dest_weight(),
			);
			let result = pallet_xcm::Pallet::<T>::send_xcm(Here, Parent, xcm_message);
			log::debug!(
				target: "homa",
				"subaccount {:?} send XCM to bond {:?} on relaychain result: {:?}",
				sub_account_index, bond_amount, result,
			);
			ensure!(result.is_ok(), Error::<T>::XcmFailed);
			Ok(())
		}

		/// Send XCM message to the relaychain to unbond subaccount.
		pub fn unbond_on_relaychain(sub_account_index: SubAccountIndex, amount: Balance) -> DispatchResult {
			let xcm_message = T::RelayChainCallBuilder::finalize_call_into_xcm_message(
				T::RelayChainCallBuilder::utility_as_derivative_call(
					T::RelayChainCallBuilder::staking_unbond(amount),
					sub_account_index,
				),
				T::XcmMessageFee::get(),
				Self::xcm_dest_weight(),
			);
			let result = pallet_xcm::Pallet::<T>::send_xcm(Here, Parent, xcm_message);
			log::debug!(
				target: "homa",
				"subaccount {:?} send XCM to unbond {:?} on relaychain result: {:?}",
				sub_account_index, amount, result
			);
			ensure!(result.is_ok(), Error::<T>::XcmFailed);
			Ok(())
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
