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

//! Homa module.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{pallet_prelude::*, transactional, PalletId};
use frame_system::{ensure_signed, pallet_prelude::*};
use module_support::{
	ExchangeRate, ExchangeRateProvider, FractionalRate, HomaManager, HomaSubAccountXcm, NomineesProvider, Rate, Ratio,
};
use orml_traits::MultiCurrency;
use primitives::{Balance, CurrencyId, EraIndex};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{
		AccountIdConversion, BlockNumberProvider, Bounded, CheckedDiv, CheckedSub, One, Saturating,
		UniqueSaturatedInto, Zero,
	},
	ArithmeticError, FixedPointNumber,
};
use sp_std::{cmp::Ordering, convert::From, prelude::*, vec, vec::Vec};

pub use module::*;
pub use weights::WeightInfo;

mod mock;
mod tests;
pub mod weights;

#[frame_support::pallet]
pub mod module {
	use super::*;

	pub type RelayChainAccountIdOf<T> = <<T as Config>::XcmInterface as HomaSubAccountXcm<
		<T as frame_system::Config>::AccountId,
		Balance,
	>>::RelayChainAccountId;

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
		pub value: Balance,
		/// Era number at which point it'll be unlocked.
		#[codec(compact)]
		pub era: EraIndex,
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
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Multi-currency support for asset management
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

		/// Origin represented Governance
		type GovernanceOrigin: EnsureOrigin<<Self as frame_system::Config>::RuntimeOrigin>;

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

		/// The staking amount of threshold to mint.
		#[pallet::constant]
		type MintThreshold: Get<Balance>;

		/// The liquid amount of threshold to redeem.
		#[pallet::constant]
		type RedeemThreshold: Get<Balance>;

		/// Block number provider for the relaychain.
		type RelayChainBlockNumber: BlockNumberProvider<BlockNumber = BlockNumberFor<Self>>;

		/// The XcmInterface to manage the staking of sub-account on relaychain.
		type XcmInterface: HomaSubAccountXcm<Self::AccountId, Balance>;

		/// The limit for process redeem requests when bump era.
		#[pallet::constant]
		type ProcessRedeemRequestsLimit: Get<u32>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;

		type NominationsProvider: NomineesProvider<RelayChainAccountIdOf<Self>>;
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
		/// The era index to bump is outdated, must be greater than RelayChainCurrentEra
		OutdatedEraIndex,
		/// Redeem request is not allowed to be fast matched.
		FastMatchIsNotAllowed,
		/// The fast match cannot be matched completely.
		CannotCompletelyFastMatch,
		// Invalid rate,
		InvalidRate,
		/// Invalid last era bumped block config
		InvalidLastEraBumpedBlock,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// The minter use staking currency to mint liquid currency.
		Minted {
			minter: T::AccountId,
			staking_currency_amount: Balance,
			liquid_amount_received: Balance,
			liquid_amount_added_to_void: Balance,
		},
		/// Request redeem.
		RequestedRedeem {
			redeemer: T::AccountId,
			liquid_amount: Balance,
			allow_fast_match: bool,
		},
		/// Redeem request has been cancelled.
		RedeemRequestCancelled {
			redeemer: T::AccountId,
			cancelled_liquid_amount: Balance,
		},
		/// Redeem request is redeemed partially or fully by fast match.
		RedeemedByFastMatch {
			redeemer: T::AccountId,
			matched_liquid_amount: Balance,
			fee_in_liquid: Balance,
			redeemed_staking_amount: Balance,
		},
		/// Redeem request is redeemed by unbond on relaychain.
		RedeemedByUnbond {
			redeemer: T::AccountId,
			era_index_when_unbond: EraIndex,
			liquid_amount: Balance,
			unbonding_staking_amount: Balance,
		},
		/// The redeemer withdraw expired redemption.
		WithdrawRedemption {
			redeemer: T::AccountId,
			redemption_amount: Balance,
		},
		/// The current era has been bumped.
		CurrentEraBumped { new_era_index: EraIndex },
		/// The current era has been reset.
		CurrentEraReset { new_era_index: EraIndex },
		/// The bonded amount of subaccount's ledger has been reset.
		LedgerBondedReset {
			sub_account_index: u16,
			new_bonded_amount: Balance,
		},
		/// The unlocking of subaccount's ledger has been reset.
		LedgerUnlockingReset {
			sub_account_index: u16,
			new_unlocking: Vec<UnlockChunk>,
		},
		/// The soft bonded cap of per sub account has been updated.
		SoftBondedCapPerSubAccountUpdated { cap_amount: Balance },
		/// The estimated reward rate per era of relaychain staking has been updated.
		EstimatedRewardRatePerEraUpdated { reward_rate: Rate },
		/// The commission rate has been updated.
		CommissionRateUpdated { commission_rate: Rate },
		/// The fast match fee rate has been updated.
		FastMatchFeeRateUpdated { fast_match_fee_rate: Rate },
		/// The relaychain block number of last era bumped updated.
		LastEraBumpedBlockUpdated { last_era_bumped_block: BlockNumberFor<T> },
		/// The frequency to bump era has been updated.
		BumpEraFrequencyUpdated { frequency: BlockNumberFor<T> },
		/// The interval eras to nominate.
		NominateIntervalEraUpdated { eras: EraIndex },
		/// Withdraw unbonded from RelayChain
		HomaWithdrawUnbonded { sub_account_index: u16, amount: Balance },
		/// Unbond staking currency of sub account on RelayChain
		HomaUnbond { sub_account_index: u16, amount: Balance },
		/// Transfer staking currency to sub account and bond on RelayChain
		HomaBondExtra { sub_account_index: u16, amount: Balance },
		/// Nominate validators on RelayChain
		HomaNominate {
			sub_account_index: u16,
			nominations: Vec<RelayChainAccountIdOf<T>>,
		},
	}

	/// The current era of relaychain
	///
	/// RelayChainCurrentEra : EraIndex
	#[pallet::storage]
	#[pallet::getter(fn relay_chain_current_era)]
	pub type RelayChainCurrentEra<T: Config> = StorageValue<_, EraIndex, ValueQuery>;

	/// The staking ledger of Homa subaccounts.
	///
	/// StakingLedgers map: u16 => Option<StakingLedger>
	#[pallet::storage]
	#[pallet::getter(fn staking_ledgers)]
	pub type StakingLedgers<T: Config> = StorageMap<_, Twox64Concat, u16, StakingLedger, OptionQuery>;

	/// The total amount of staking currency bonded in the homa protocol
	///
	/// TotalStakingBonded value: Balance
	#[pallet::storage]
	#[pallet::getter(fn get_total_bonded)]
	pub type TotalStakingBonded<T: Config> = StorageValue<_, Balance, ValueQuery>;

	/// The total staking currency to bond on relaychain when new era,
	/// and that is available to be match fast redeem request.
	/// ToBondPool value: StakingCurrencyAmount
	#[pallet::storage]
	#[pallet::getter(fn to_bond_pool)]
	pub type ToBondPool<T: Config> = StorageValue<_, Balance, ValueQuery>;

	/// The total amount of void liquid currency. It's will not be issued,
	/// used to avoid newly issued LDOT to obtain the incoming staking income from relaychain.
	/// And it is guaranteed that the current exchange rate between liquid currency and staking
	/// currency will not change. It will be reset to 0 at the beginning of the `rebalance` when new
	/// era starts.
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
	/// Unbondings: double_map AccountId, ExpireEraIndex => UnbondingStakingCurrencyAmount
	#[pallet::storage]
	#[pallet::getter(fn unbondings)]
	pub type Unbondings<T: Config> =
		StorageDoubleMap<_, Twox64Concat, T::AccountId, Twox64Concat, EraIndex, Balance, ValueQuery>;

	/// The estimated staking reward rate per era on relaychain.
	///
	/// EstimatedRewardRatePerEra: value: Rate
	#[pallet::storage]
	pub type EstimatedRewardRatePerEra<T: Config> = StorageValue<_, FractionalRate, ValueQuery>;

	/// The maximum amount of bonded staking currency for a single sub on relaychain to obtain the
	/// best staking rewards.
	///
	/// SoftBondedCapPerSubAccount: value: Balance
	#[pallet::storage]
	#[pallet::getter(fn soft_bonded_cap_per_sub_account)]
	pub type SoftBondedCapPerSubAccount<T: Config> = StorageValue<_, Balance, ValueQuery>;

	/// The rate of Homa drawn from the staking reward as commission.
	/// The draw will be transfer to TreasuryAccount of Homa in liquid currency.
	///
	/// CommissionRate: value: Rate
	#[pallet::storage]
	pub type CommissionRate<T: Config> = StorageValue<_, FractionalRate, ValueQuery>;

	/// The fixed fee rate for redeem request is fast matched.
	///
	/// FastMatchFeeRate: value: Rate
	#[pallet::storage]
	pub type FastMatchFeeRate<T: Config> = StorageValue<_, FractionalRate, ValueQuery>;

	/// The relaychain block number of last era bumped.
	///
	/// LastEraBumpedBlock: value: BlockNumberFor<T>
	#[pallet::storage]
	#[pallet::getter(fn last_era_bumped_block)]
	pub type LastEraBumpedBlock<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

	/// The interval of relaychain block number of relaychain to bump local current era.
	///
	/// LastEraBumpedRelayChainBlock: value: BlockNumberFor<T>
	#[pallet::storage]
	#[pallet::getter(fn bump_era_frequency)]
	pub type BumpEraFrequency<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

	/// The interval of eras to nominate on relaychain.
	///
	/// NominateIntervalEra: value: EraIndex
	#[pallet::storage]
	#[pallet::getter(fn nominate_interval_era)]
	pub type NominateIntervalEra<T: Config> = StorageValue<_, EraIndex, ValueQuery>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(_: BlockNumberFor<T>) -> Weight {
			let bump_era_number = Self::era_amount_should_to_bump(T::RelayChainBlockNumber::current_block_number());
			if !bump_era_number.is_zero() {
				let res = Self::bump_current_era(bump_era_number);
				debug_assert_eq!(
					TotalStakingBonded::<T>::get(),
					StakingLedgers::<T>::iter().fold(Zero::zero(), |total_bonded: Balance, (_, ledger)| {
						total_bonded.saturating_add(ledger.bonded)
					})
				);
				<T as Config>::WeightInfo::on_initialize_with_bump_era(res.unwrap_or_default())
			} else {
				<T as Config>::WeightInfo::on_initialize()
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Mint liquid currency by put locking up amount of staking currency.
		///
		/// Parameters:
		/// - `amount`: The amount of staking currency used to mint liquid currency.
		#[pallet::call_index(0)]
		#[pallet::weight(< T as Config >::WeightInfo::mint())]
		pub fn mint(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResult {
			let minter = ensure_signed(origin)?;
			Self::do_mint(minter, amount)
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
		#[pallet::call_index(1)]
		#[pallet::weight(< T as Config >::WeightInfo::request_redeem())]
		pub fn request_redeem(
			origin: OriginFor<T>,
			#[pallet::compact] amount: Balance,
			allow_fast_match: bool,
		) -> DispatchResult {
			let redeemer = ensure_signed(origin)?;
			Self::do_request_redeem(redeemer, amount, allow_fast_match)
		}

		/// Execute fast match for specific redeem requests.
		///
		/// Parameters:
		/// - `redeemer_list`: The list of redeem requests to execute fast redeem.
		#[pallet::call_index(2)]
		#[pallet::weight(< T as Config >::WeightInfo::fast_match_redeems(redeemer_list.len() as u32))]
		pub fn fast_match_redeems(origin: OriginFor<T>, redeemer_list: Vec<T::AccountId>) -> DispatchResult {
			let _ = ensure_signed(origin)?;

			for redeemer in redeemer_list {
				Self::do_fast_match_redeem(&redeemer, true)?;
			}

			Ok(())
		}

		/// Withdraw the expired redemption of specific redeemer by unbond.
		///
		/// Parameters:
		/// - `redeemer`: redeemer.
		#[pallet::call_index(3)]
		#[pallet::weight(< T as Config >::WeightInfo::claim_redemption())]
		pub fn claim_redemption(origin: OriginFor<T>, redeemer: T::AccountId) -> DispatchResult {
			let _ = ensure_signed(origin)?;

			let mut available_staking: Balance = Zero::zero();
			let current_era = Self::relay_chain_current_era();
			for (expired_era_index, unbonded) in Unbondings::<T>::iter_prefix(&redeemer) {
				if expired_era_index <= current_era {
					available_staking = available_staking.saturating_add(unbonded);
					Unbondings::<T>::remove(&redeemer, expired_era_index);
				}
			}

			if !available_staking.is_zero() {
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

				Self::deposit_event(Event::<T>::WithdrawRedemption {
					redeemer,
					redemption_amount: available_staking,
				});
			}

			Ok(())
		}

		/// Sets the params of Homa.
		/// Requires `GovernanceOrigin`
		///
		/// Parameters:
		/// - `soft_bonded_cap_per_sub_account`:  soft cap of staking amount for a single nominator
		///   on relaychain to obtain the best staking rewards.
		/// - `estimated_reward_rate_per_era`: the estimated staking yield of each era on the
		///   current relay chain.
		/// - `commission_rate`: the rate to draw from estimated staking rewards as commission to
		///   HomaTreasury
		/// - `fast_match_fee_rate`: the fixed fee rate when redeem request is been fast matched.
		#[pallet::call_index(4)]
		#[pallet::weight(< T as Config >::WeightInfo::update_homa_params())]
		pub fn update_homa_params(
			origin: OriginFor<T>,
			soft_bonded_cap_per_sub_account: Option<Balance>,
			estimated_reward_rate_per_era: Option<Rate>,
			commission_rate: Option<Rate>,
			fast_match_fee_rate: Option<Rate>,
			nominate_interval_era: Option<EraIndex>,
		) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			if let Some(cap_amount) = soft_bonded_cap_per_sub_account {
				SoftBondedCapPerSubAccount::<T>::put(cap_amount);
				Self::deposit_event(Event::<T>::SoftBondedCapPerSubAccountUpdated { cap_amount });
			}
			if let Some(reward_rate) = estimated_reward_rate_per_era {
				EstimatedRewardRatePerEra::<T>::mutate(|rate| -> DispatchResult {
					rate.try_set(reward_rate).map_err(|_| Error::<T>::InvalidRate.into())
				})?;
				Self::deposit_event(Event::<T>::EstimatedRewardRatePerEraUpdated { reward_rate });
			}
			if let Some(commission_rate) = commission_rate {
				CommissionRate::<T>::mutate(|rate| -> DispatchResult {
					rate.try_set(commission_rate)
						.map_err(|_| Error::<T>::InvalidRate.into())
				})?;
				Self::deposit_event(Event::<T>::CommissionRateUpdated { commission_rate });
			}
			if let Some(fast_match_fee_rate) = fast_match_fee_rate {
				FastMatchFeeRate::<T>::mutate(|rate| -> DispatchResult {
					rate.try_set(fast_match_fee_rate)
						.map_err(|_| Error::<T>::InvalidRate.into())
				})?;
				Self::deposit_event(Event::<T>::FastMatchFeeRateUpdated { fast_match_fee_rate });
			}
			if let Some(interval) = nominate_interval_era {
				NominateIntervalEra::<T>::set(interval);
				Self::deposit_event(Event::<T>::NominateIntervalEraUpdated { eras: interval });
			}

			Ok(())
		}

		/// Sets the params that control when to bump local current era.
		/// Requires `GovernanceOrigin`
		///
		/// Parameters:
		/// - `fix_last_era_bumped_block`: fix the relaychain block number of last era bumped.
		/// - `frequency`: the frequency of block number on parachain.
		#[pallet::call_index(5)]
		#[pallet::weight(< T as Config >::WeightInfo::update_bump_era_params())]
		pub fn update_bump_era_params(
			origin: OriginFor<T>,
			last_era_bumped_block: Option<BlockNumberFor<T>>,
			frequency: Option<BlockNumberFor<T>>,
		) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			if let Some(change) = frequency {
				BumpEraFrequency::<T>::put(change);
				Self::deposit_event(Event::<T>::BumpEraFrequencyUpdated { frequency: change });
			}

			if let Some(change) = last_era_bumped_block {
				// config last_era_bumped_block should not cause bump era to occur immediately, because
				// the last_era_bumped_block after the bump era will not be same with the actual relaychain
				// era bumped block  again, especially if it leads to multiple bump era.
				// and it should be config after config no-zero bump_era_frequency.
				let bump_era_frequency = Self::bump_era_frequency();
				let current_relay_chain_block = T::RelayChainBlockNumber::current_block_number();
				if !bump_era_frequency.is_zero() {
					// ensure change in this range (current_relay_chain_block-bump_era_frequency,
					// current_relay_chain_block]
					ensure!(
						change > current_relay_chain_block.saturating_sub(bump_era_frequency)
							&& change <= current_relay_chain_block,
						Error::<T>::InvalidLastEraBumpedBlock
					);

					LastEraBumpedBlock::<T>::put(change);
					Self::deposit_event(Event::<T>::LastEraBumpedBlockUpdated {
						last_era_bumped_block: change,
					});
				}
			}

			Ok(())
		}

		/// Reset the bonded and unbonding to local subaccounts ledger according to the ledger on
		/// relaychain. Requires `GovernanceOrigin`
		///
		/// Parameters:
		/// - `updates`: update list of subaccount.
		#[pallet::call_index(6)]
		#[pallet::weight(< T as Config >::WeightInfo::reset_ledgers(updates.len() as u32))]
		pub fn reset_ledgers(
			origin: OriginFor<T>,
			updates: Vec<(u16, Option<Balance>, Option<Vec<UnlockChunk>>)>,
		) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			for (sub_account_index, bonded_change, unlocking_change) in updates {
				Self::do_update_ledger(sub_account_index, |ledger| -> DispatchResult {
					if let Some(change) = bonded_change {
						if ledger.bonded != change {
							ledger.bonded = change;
							Self::deposit_event(Event::<T>::LedgerBondedReset {
								sub_account_index,
								new_bonded_amount: change,
							});
						}
					}
					if let Some(change) = unlocking_change {
						if ledger.unlocking != change {
							ledger.unlocking = change.clone();
							Self::deposit_event(Event::<T>::LedgerUnlockingReset {
								sub_account_index,
								new_unlocking: change,
							});
						}
					}
					Ok(())
				})?;
			}

			Ok(())
		}

		/// Reset the RelayChainCurrentEra.
		/// If there is a deviation of more than 1 EraIndex between current era of relaychain and
		/// current era on local, should reset era to current era of relaychain as soon as possible.
		/// At the same time, check whether the unlocking of ledgers should be updated.
		/// Requires `GovernanceOrigin`
		///
		/// Parameters:
		/// - `era_index`: the latest era index of relaychain.
		#[pallet::call_index(7)]
		#[pallet::weight(< T as Config >::WeightInfo::reset_current_era())]
		pub fn reset_current_era(origin: OriginFor<T>, era_index: EraIndex) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			RelayChainCurrentEra::<T>::mutate(|current_era| {
				if *current_era != era_index {
					*current_era = era_index;
					Self::deposit_event(Event::<T>::CurrentEraReset {
						new_era_index: era_index,
					});
				}
			});

			Ok(())
		}

		#[pallet::call_index(8)]
		#[pallet::weight(< T as Config >::WeightInfo::on_initialize_with_bump_era(T::ProcessRedeemRequestsLimit::get()))]
		pub fn force_bump_current_era(origin: OriginFor<T>, bump_amount: EraIndex) -> DispatchResultWithPostInfo {
			T::GovernanceOrigin::ensure_origin(origin)?;

			let res = Self::bump_current_era(bump_amount);
			Ok(Some(T::WeightInfo::on_initialize_with_bump_era(res.unwrap_or_default())).into())
		}

		/// Execute fast match for specific redeem requests, require completely matched.
		///
		/// Parameters:
		/// - `redeemer_list`: The list of redeem requests to execute fast redeem.
		#[pallet::call_index(9)]
		#[pallet::weight(< T as Config >::WeightInfo::fast_match_redeems(redeemer_list.len() as u32))]
		pub fn fast_match_redeems_completely(origin: OriginFor<T>, redeemer_list: Vec<T::AccountId>) -> DispatchResult {
			let _ = ensure_signed(origin)?;

			for redeemer in redeemer_list {
				Self::do_fast_match_redeem(&redeemer, false)?;
			}

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Module account id
		pub fn account_id() -> T::AccountId {
			T::PalletId::get().into_account_truncating()
		}

		pub(crate) fn estimated_reward_rate_per_era() -> Rate {
			EstimatedRewardRatePerEra::<T>::get().into_inner()
		}

		pub(crate) fn commission_rate() -> Rate {
			CommissionRate::<T>::get().into_inner()
		}

		pub(crate) fn fast_match_fee_rate() -> Rate {
			FastMatchFeeRate::<T>::get().into_inner()
		}

		pub fn do_update_ledger<R, E>(
			sub_account_index: u16,
			f: impl FnOnce(&mut StakingLedger) -> sp_std::result::Result<R, E>,
		) -> sp_std::result::Result<R, E> {
			StakingLedgers::<T>::try_mutate_exists(sub_account_index, |maybe_ledger| {
				let mut ledger = maybe_ledger.take().unwrap_or_default();
				let old_bonded_amount = ledger.bonded;

				f(&mut ledger).map(move |result| {
					*maybe_ledger = if ledger == Default::default() {
						TotalStakingBonded::<T>::mutate(|staking_balance| {
							*staking_balance = staking_balance.saturating_sub(old_bonded_amount)
						});
						None
					} else {
						TotalStakingBonded::<T>::mutate(|staking_balance| {
							*staking_balance = staking_balance
								.saturating_add(ledger.bonded)
								.saturating_sub(old_bonded_amount)
						});
						Some(ledger)
					};
					result
				})
			})
		}

		pub(super) fn do_mint(minter: T::AccountId, amount: Balance) -> DispatchResult {
			// Ensure the amount is above the MintThreshold.
			ensure!(amount >= T::MintThreshold::get(), Error::<T>::BelowMintThreshold);

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

			Self::issue_liquid_currency(&minter, liquid_issue_to_minter)?;

			ToBondPool::<T>::mutate(|pool| *pool = pool.saturating_add(amount));
			TotalVoidLiquid::<T>::mutate(|total| *total = total.saturating_add(liquid_add_to_void));

			Self::deposit_event(Event::<T>::Minted {
				minter,
				staking_currency_amount: amount,
				liquid_amount_received: liquid_issue_to_minter,
				liquid_amount_added_to_void: liquid_add_to_void,
			});
			Ok(())
		}

		pub(super) fn do_request_redeem(
			redeemer: T::AccountId,
			amount: Balance,
			allow_fast_match: bool,
		) -> DispatchResult {
			RedeemRequests::<T>::try_mutate_exists(&redeemer, |maybe_request| -> DispatchResult {
				let (previous_request_amount, _) = maybe_request.take().unwrap_or_default();
				let liquid_currency_id = T::LiquidCurrencyId::get();

				ensure!(
					(!previous_request_amount.is_zero() && amount.is_zero()) || amount >= T::RedeemThreshold::get(),
					Error::<T>::BelowRedeemThreshold
				);

				match amount.cmp(&previous_request_amount) {
					Ordering::Greater => {
						// pay more liquid currency.
						T::Currency::transfer(
							liquid_currency_id,
							&redeemer,
							&Self::account_id(),
							amount.saturating_sub(previous_request_amount),
						)
					}
					Ordering::Less => {
						// refund the difference.
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
					Self::deposit_event(Event::<T>::RequestedRedeem {
						redeemer: redeemer.clone(),
						liquid_amount: amount,
						allow_fast_match,
					});
				} else if !previous_request_amount.is_zero() {
					Self::deposit_event(Event::<T>::RedeemRequestCancelled {
						redeemer: redeemer.clone(),
						cancelled_liquid_amount: previous_request_amount,
					});
				}
				Ok(())
			})
		}

		/// Get the soft cap of total staking currency of Homa.
		/// Soft cap = ActiveSubAccountsIndexList.len() * SoftBondedCapPerSubAccount
		pub fn get_staking_currency_soft_cap() -> Balance {
			Self::soft_bonded_cap_per_sub_account()
				.saturating_mul(T::ActiveSubAccountsIndexList::get().len() as Balance)
		}

		/// Calculate the total amount of staking currency belong to Homa.
		pub fn get_total_staking_currency() -> Balance {
			TotalStakingBonded::<T>::get().saturating_add(Self::to_bond_pool())
		}

		/// Calculate the total amount of liquid currency.
		/// total_liquid_amount = total issuance of LiquidCurrencyId + TotalVoidLiquid
		pub fn get_total_liquid_currency() -> Balance {
			T::Currency::total_issuance(T::LiquidCurrencyId::get()).saturating_add(Self::total_void_liquid())
		}

		/// Calculate the current exchange rate between the staking currency and liquid currency.
		/// Note: ExchangeRate(staking : liquid) = total_staking_amount / total_liquid_amount.
		/// If the exchange rate cannot be calculated, T::DefaultExchangeRate is used.
		pub fn current_exchange_rate() -> ExchangeRate {
			let total_staking = Self::get_total_staking_currency();
			let total_liquid = Self::get_total_liquid_currency();
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
		pub fn do_fast_match_redeem(redeemer: &T::AccountId, allow_partially: bool) -> DispatchResult {
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

					// calculate the actual liquid currency to be used to redeem
					let actual_liquid_to_redeem = if liquid_limit_at_fee_rate >= request_amount {
						request_amount
					} else {
						// if cannot fast match the request amount fully, at least keep RedeemThreshold as remainder.
						liquid_limit_at_fee_rate.min(request_amount.saturating_sub(T::RedeemThreshold::get()))
					};

					if !actual_liquid_to_redeem.is_zero() {
						let liquid_to_burn = Rate::one()
							.saturating_sub(fast_match_fee_rate)
							.saturating_mul_int(actual_liquid_to_redeem);
						let redeemed_staking = Self::convert_liquid_to_staking(liquid_to_burn)?;
						let fee_in_liquid = actual_liquid_to_redeem.saturating_sub(liquid_to_burn);

						// burn liquid_to_burn for redeemed_staking and burn fee_in_liquid to reward all holders of
						// liquid currency.
						Self::burn_liquid_currency(&module_account, actual_liquid_to_redeem)?;

						// transfer redeemed_staking to redeemer.
						T::Currency::transfer(
							T::StakingCurrencyId::get(),
							&module_account,
							redeemer,
							redeemed_staking,
						)?;
						ToBondPool::<T>::mutate(|pool| *pool = pool.saturating_sub(redeemed_staking));

						Self::deposit_event(Event::<T>::RedeemedByFastMatch {
							redeemer: redeemer.clone(),
							matched_liquid_amount: actual_liquid_to_redeem,
							fee_in_liquid,
							redeemed_staking_amount: redeemed_staking,
						});
					}

					// update request amount
					let remainder_request_amount = request_amount.saturating_sub(actual_liquid_to_redeem);
					if !remainder_request_amount.is_zero() {
						ensure!(allow_partially, Error::<T>::CannotCompletelyFastMatch);
						*maybe_request = Some((remainder_request_amount, allow_fast_match));
					}
				}

				Ok(())
			})
		}

		/// Accumulate staking rewards according to EstimatedRewardRatePerEra and era internally.
		/// And draw commission from estimated staking rewards by issuing liquid currency to
		/// TreasuryAccount. Note: This will cause some losses to the minters in previous_era,
		/// because they have been already deducted some liquid currency amount when mint in
		/// previous_era. Until there is a better way to calculate, this part of the loss can only
		/// be regarded as an implicit mint fee!
		#[transactional]
		pub fn process_staking_rewards(new_era: EraIndex, previous_era: EraIndex) -> DispatchResult {
			let era_interval = new_era.saturating_sub(previous_era);
			let reward_rate = Self::estimated_reward_rate_per_era()
				.saturating_add(Rate::one())
				.saturating_pow(era_interval.unique_saturated_into())
				.saturating_sub(Rate::one());

			if !reward_rate.is_zero() {
				let mut total_reward_staking: Balance = Zero::zero();

				// iterate all subaccounts
				for (sub_account_index, ledger) in StakingLedgers::<T>::iter() {
					let reward_staking = reward_rate.saturating_mul_int(ledger.bonded);

					if !reward_staking.is_zero() {
						Self::do_update_ledger(sub_account_index, |before| -> DispatchResult {
							before.bonded = before.bonded.saturating_add(reward_staking);
							Ok(())
						})?;

						total_reward_staking = total_reward_staking.saturating_add(reward_staking);
					}
				}

				let commission_rate = Self::commission_rate();
				if !total_reward_staking.is_zero() && !commission_rate.is_zero() {
					let commission_staking_amount = commission_rate.saturating_mul_int(total_reward_staking);
					let commission_ratio =
						Ratio::checked_from_rational(commission_staking_amount, TotalStakingBonded::<T>::get())
							.unwrap_or_else(Ratio::min_value);
					let inflate_rate = commission_ratio
						.checked_div(&Ratio::one().saturating_sub(commission_ratio))
						.unwrap_or_else(Ratio::max_value);
					let inflate_liquid_amount = inflate_rate.saturating_mul_int(Self::get_total_liquid_currency());

					Self::issue_liquid_currency(&T::TreasuryAccount::get(), inflate_liquid_amount)?;
				}
			}

			Ok(())
		}

		/// Get back unbonded of all subaccounts on relaychain by XCM.
		/// The staking currency withdrew becomes available to be redeemed.
		#[transactional]
		pub fn process_scheduled_unbond(new_era: EraIndex) -> DispatchResult {
			let mut total_withdrawn_staking: Balance = Zero::zero();

			// iterate all subaccounts
			for (sub_account_index, ledger) in StakingLedgers::<T>::iter() {
				let (new_ledger, expired_unlocking) = ledger.consolidate_unlocked(new_era);

				if !expired_unlocking.is_zero() {
					T::XcmInterface::withdraw_unbonded_from_sub_account(sub_account_index, expired_unlocking)?;

					// update ledger
					Self::do_update_ledger(sub_account_index, |before| -> DispatchResult {
						*before = new_ledger;
						Ok(())
					})?;
					total_withdrawn_staking = total_withdrawn_staking.saturating_add(expired_unlocking);

					Self::deposit_event(Event::<T>::HomaWithdrawUnbonded {
						sub_account_index,
						amount: expired_unlocking,
					});
				}
			}

			// issue withdrawn unbonded to module account for redeemer to claim
			Self::issue_staking_currency(&Self::account_id(), total_withdrawn_staking)?;
			UnclaimedRedemption::<T>::mutate(|total| *total = total.saturating_add(total_withdrawn_staking));

			Ok(())
		}

		/// Distribute PoolToBond to ActiveSubAccountsIndexList, then cross-transfer the
		/// distribution amount to the subaccounts on relaychain and bond it by XCM.
		#[transactional]
		pub fn process_to_bond_pool() -> DispatchResult {
			let to_bond_pool = Self::to_bond_pool();

			// if to_bond is gte than MintThreshold, try to bond_extra on relaychain
			if to_bond_pool >= T::MintThreshold::get() {
				let xcm_transfer_fee = T::XcmInterface::get_xcm_transfer_fee();
				let bonded_list: Vec<(u16, Balance)> = T::ActiveSubAccountsIndexList::get()
					.iter()
					.map(|index| (*index, Self::staking_ledgers(index).unwrap_or_default().bonded))
					.collect();
				let (distribution, remainder) = distribute_increment::<u16>(
					bonded_list,
					to_bond_pool,
					Some(Self::soft_bonded_cap_per_sub_account().saturating_add(xcm_transfer_fee)),
					Some(xcm_transfer_fee),
				);

				// subaccounts execute the distribution
				for (sub_account_index, amount) in distribution {
					if !amount.is_zero() {
						T::XcmInterface::transfer_staking_to_sub_account(
							&Self::account_id(),
							sub_account_index,
							amount,
						)?;

						let bond_amount = amount.saturating_sub(xcm_transfer_fee);
						T::XcmInterface::bond_extra_on_sub_account(sub_account_index, bond_amount)?;

						// update ledger
						Self::do_update_ledger(sub_account_index, |ledger| -> DispatchResult {
							ledger.bonded = ledger.bonded.saturating_add(bond_amount);
							Ok(())
						})?;

						Self::deposit_event(Event::<T>::HomaBondExtra {
							sub_account_index,
							amount: bond_amount,
						});
					}
				}

				// update pool
				ToBondPool::<T>::mutate(|pool| *pool = remainder);
			}

			Ok(())
		}

		/// Process redeem requests and subaccounts do unbond on relaychain by XCM message.
		#[transactional]
		pub fn process_redeem_requests(new_era: EraIndex) -> Result<u32, DispatchError> {
			let era_index_to_expire = new_era + T::BondingDuration::get();
			let total_bonded = TotalStakingBonded::<T>::get();
			let mut total_redeem_amount: Balance = Zero::zero();
			let mut remain_total_bonded = total_bonded;
			let mut handled_requests: u32 = 0;

			// iter RedeemRequests and insert to Unbondings if remain_total_bonded is enough.
			for (redeemer, (redeem_amount, _)) in RedeemRequests::<T>::iter() {
				let redemption_amount = Self::convert_liquid_to_staking(redeem_amount)?;

				if remain_total_bonded >= redemption_amount && handled_requests < T::ProcessRedeemRequestsLimit::get() {
					total_redeem_amount = total_redeem_amount.saturating_add(redeem_amount);
					remain_total_bonded = remain_total_bonded.saturating_sub(redemption_amount);
					RedeemRequests::<T>::remove(&redeemer);
					Unbondings::<T>::mutate(&redeemer, era_index_to_expire, |n| {
						*n = n.saturating_add(redemption_amount)
					});
					Self::deposit_event(Event::<T>::RedeemedByUnbond {
						redeemer,
						era_index_when_unbond: new_era,
						liquid_amount: redeem_amount,
						unbonding_staking_amount: redemption_amount,
					});

					handled_requests += 1;
				} else {
					break;
				}
			}

			// calculate the distribution for unbond
			let staking_amount_to_unbond = total_bonded.saturating_sub(remain_total_bonded);
			let bonded_list: Vec<(u16, Balance)> = T::ActiveSubAccountsIndexList::get()
				.iter()
				.map(|index| (*index, Self::staking_ledgers(index).unwrap_or_default().bonded))
				.collect();
			let (distribution, _) = distribute_decrement::<u16>(bonded_list, staking_amount_to_unbond, None, None);

			// subaccounts execute the distribution
			for (sub_account_index, unbond_amount) in distribution {
				if !unbond_amount.is_zero() {
					T::XcmInterface::unbond_on_sub_account(sub_account_index, unbond_amount)?;

					// update ledger
					Self::do_update_ledger(sub_account_index, |ledger| -> DispatchResult {
						ledger.bonded = ledger.bonded.saturating_sub(unbond_amount);
						ledger.unlocking.push(UnlockChunk {
							value: unbond_amount,
							era: era_index_to_expire,
						});
						Ok(())
					})?;

					Self::deposit_event(Event::<T>::HomaUnbond {
						sub_account_index,
						amount: unbond_amount,
					});
				}
			}

			// burn total_redeem_amount.
			Self::burn_liquid_currency(&Self::account_id(), total_redeem_amount)?;

			Ok(handled_requests)
		}

		/// Process nominate validators for subaccounts on relaychain.
		pub fn process_nominate(new_era: EraIndex) -> DispatchResult {
			// check whether need to nominate
			let nominate_interval_era = NominateIntervalEra::<T>::get();
			if !nominate_interval_era.is_zero() && new_era % nominate_interval_era == 0 {
				for (sub_account_index, nominations) in
					T::NominationsProvider::nominees_in_groups(T::ActiveSubAccountsIndexList::get())
				{
					if !nominations.is_empty() {
						T::XcmInterface::nominate_on_sub_account(sub_account_index, nominations.clone())?;

						Self::deposit_event(Event::<T>::HomaNominate {
							sub_account_index,
							nominations,
						});
					}
				}
			}

			Ok(())
		}

		pub fn era_amount_should_to_bump(relaychain_block_number: BlockNumberFor<T>) -> EraIndex {
			relaychain_block_number
				.checked_sub(&Self::last_era_bumped_block())
				.and_then(|n| n.checked_div(&Self::bump_era_frequency()))
				.and_then(|n| TryInto::<EraIndex>::try_into(n).ok())
				.unwrap_or_else(Zero::zero)
		}

		/// Bump current era.
		/// The rebalance will send XCM messages to relaychain. Once the XCM message is sent,
		/// the execution result cannot be obtained and cannot be rolled back. So the process
		/// of rebalance is not atomic.
		pub fn bump_current_era(amount: EraIndex) -> Result<u32, DispatchError> {
			let previous_era = Self::relay_chain_current_era();
			let new_era = previous_era.saturating_add(amount);
			RelayChainCurrentEra::<T>::put(new_era);
			LastEraBumpedBlock::<T>::put(T::RelayChainBlockNumber::current_block_number());
			Self::deposit_event(Event::<T>::CurrentEraBumped { new_era_index: new_era });

			// Rebalance:
			let res = || -> Result<u32, DispatchError> {
				TotalVoidLiquid::<T>::put(0);
				Self::process_staking_rewards(new_era, previous_era)?;
				Self::process_scheduled_unbond(new_era)?;
				Self::process_to_bond_pool()?;
				let count = Self::process_redeem_requests(new_era)?;
				Self::process_nominate(new_era)?;
				Ok(count)
			}();

			log::debug!(
				target: "homa",
				"bump era to {:?}, rebalance result is {:?}",
				new_era, res
			);

			res
		}

		/// This should be the only function in the system that issues liquid currency
		fn issue_liquid_currency(who: &T::AccountId, amount: Balance) -> DispatchResult {
			T::Currency::deposit(T::LiquidCurrencyId::get(), who, amount)
		}

		/// This should be the only function in the system that burn liquid currency
		fn burn_liquid_currency(who: &T::AccountId, amount: Balance) -> DispatchResult {
			T::Currency::withdraw(T::LiquidCurrencyId::get(), who, amount)
		}

		/// Issue staking currency based on the subaccounts transfer the unbonded staking currency
		/// to the parachain account
		fn issue_staking_currency(who: &T::AccountId, amount: Balance) -> DispatchResult {
			T::Currency::deposit(T::StakingCurrencyId::get(), who, amount)
		}
	}
}

impl<T: Config> ExchangeRateProvider for Pallet<T> {
	fn get_exchange_rate() -> ExchangeRate {
		Self::current_exchange_rate()
	}
}

impl<T: Config> Get<EraIndex> for Pallet<T> {
	fn get() -> EraIndex {
		Self::relay_chain_current_era()
	}
}

impl<T: Config> HomaManager<T::AccountId, Balance> for Pallet<T> {
	fn mint(who: T::AccountId, amount: Balance) -> DispatchResult {
		Self::do_mint(who, amount)
	}

	fn request_redeem(who: T::AccountId, amount: Balance, fast_match: bool) -> DispatchResult {
		Self::do_request_redeem(who, amount, fast_match)
	}

	fn get_exchange_rate() -> ExchangeRate {
		Self::current_exchange_rate()
	}

	fn get_estimated_reward_rate() -> Rate {
		EstimatedRewardRatePerEra::<T>::get().into_inner()
	}

	fn get_commission_rate() -> Rate {
		CommissionRate::<T>::get().into_inner()
	}

	fn get_fast_match_fee() -> Rate {
		FastMatchFeeRate::<T>::get().into_inner()
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
		if remain_increment.is_zero() || remain_increment < minimum_increment.unwrap_or_else(Bounded::min_value) {
			break;
		}

		let increment_distribution = amount_cap
			.unwrap_or_else(Bounded::max_value)
			.saturating_sub(amount)
			.min(remain_increment);
		if increment_distribution.is_zero()
			|| increment_distribution < minimum_increment.unwrap_or_else(Bounded::min_value)
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
	amount_remainder: Option<Balance>,
	minimum_decrement: Option<Balance>,
) -> (Vec<(Index, Balance)>, Balance) {
	let mut remain_decrement = total_decrement;
	let mut distribution_list: Vec<(Index, Balance)> = vec![];

	// Sort by amount in descending order
	amount_list.sort_by(|a, b| b.1.cmp(&a.1));

	for (index, amount) in amount_list {
		if remain_decrement.is_zero() || remain_decrement < minimum_decrement.unwrap_or_else(Bounded::min_value) {
			break;
		}

		let decrement_distribution = amount
			.saturating_sub(amount_remainder.unwrap_or_else(Bounded::min_value))
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
