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

//! # CDP Engine Module
//!
//! ## Overview
//!
//! The core module of Honzon protocol. CDP engine is responsible for handle
//! internal processes about CDPs, including liquidation, settlement and risk
//! management.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::upper_case_acronyms)]

use frame_support::{pallet_prelude::*, traits::UnixTime, transactional, BoundedVec, PalletId};
use frame_system::{
	offchain::{SendTransactionTypes, SubmitTransaction},
	pallet_prelude::*,
};
use module_support::{
	AddressMapping, CDPTreasury, CDPTreasuryExtended, DEXManager, EVMBridge, EmergencyShutdown, ExchangeRate,
	FractionalRate, InvokeContext, LiquidateCollateral, LiquidationEvmBridge, Price, PriceProvider, Rate, Ratio,
	RiskManager, Swap, SwapLimit,
};
use orml_traits::{Change, GetByKey, MultiCurrency};
use orml_utilities::OffchainErr;
use parity_scale_codec::MaxEncodedLen;
use primitives::{evm::EvmAddress, Amount, Balance, CurrencyId, Position};
use rand_chacha::{
	rand_core::{RngCore, SeedableRng},
	ChaChaRng,
};
use scale_info::TypeInfo;
use sp_runtime::{
	offchain::{
		storage::StorageValueRef,
		storage_lock::{StorageLock, Time},
		Duration,
	},
	traits::{
		AccountIdConversion, BlockNumberProvider, Bounded, One, Saturating, StaticLookup, UniqueSaturatedInto, Zero,
	},
	transaction_validity::{
		InvalidTransaction, TransactionPriority, TransactionSource, TransactionValidity, ValidTransaction,
	},
	ArithmeticError, DispatchError, DispatchResult, FixedPointNumber, RuntimeDebug,
};
use sp_std::{marker::PhantomData, prelude::*};

mod mock;
mod tests;
pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

pub const OFFCHAIN_WORKER_DATA: &[u8] = b"acala/cdp-engine/data/";
pub const OFFCHAIN_WORKER_LOCK: &[u8] = b"acala/cdp-engine/lock/";
pub const OFFCHAIN_WORKER_MAX_ITERATIONS: &[u8] = b"acala/cdp-engine/max-iterations/";
pub const LOCK_DURATION: u64 = 100;
pub const DEFAULT_MAX_ITERATIONS: u32 = 1000;

pub type LoansOf<T> = module_loans::Pallet<T>;
pub type CurrencyOf<T> = <T as Config>::Currency;

/// Risk management params
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, Default, TypeInfo, MaxEncodedLen)]
pub struct RiskManagementParams {
	/// Maximum total debit value generated from it, when reach the hard
	/// cap, CDP's owner cannot issue more stablecoin under the collateral
	/// type.
	pub maximum_total_debit_value: Balance,

	/// Extra interest rate per sec, `None` value means not set
	pub interest_rate_per_sec: Option<FractionalRate>,

	/// Liquidation ratio, when the collateral ratio of
	/// CDP under this collateral type is below the liquidation ratio, this
	/// CDP is unsafe and can be liquidated. `None` value means not set
	pub liquidation_ratio: Option<Ratio>,

	/// Liquidation penalty rate, when liquidation occurs,
	/// CDP will be deducted an additional penalty base on the product of
	/// penalty rate and debit value. `None` value means not set
	pub liquidation_penalty: Option<FractionalRate>,

	/// Required collateral ratio, if it's set, cannot adjust the position
	/// of CDP so that the current collateral ratio is lower than the
	/// required collateral ratio. `None` value means not set
	pub required_collateral_ratio: Option<Ratio>,
}

// typedef to help polkadot.js disambiguate Change with different generic
// parameters
type ChangeOptionRate = Change<Option<Rate>>;
type ChangeOptionRatio = Change<Option<Ratio>>;
type ChangeBalance = Change<Balance>;

/// Status of CDP
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub enum CDPStatus {
	Safe,
	Unsafe,
	ChecksFailed(DispatchError),
}

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config + module_loans::Config + SendTransactionTypes<Call<Self>> {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The origin which may update risk management parameters. Root can
		/// always do this.
		type UpdateOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// The default liquidation ratio for all collateral types of CDP
		#[pallet::constant]
		type DefaultLiquidationRatio: Get<Ratio>;

		/// The default debit exchange rate for all collateral types
		#[pallet::constant]
		type DefaultDebitExchangeRate: Get<ExchangeRate>;

		/// The default liquidation penalty rate when liquidate unsafe CDP
		#[pallet::constant]
		type DefaultLiquidationPenalty: Get<FractionalRate>;

		/// The minimum debit value to avoid debit dust
		#[pallet::constant]
		type MinimumDebitValue: Get<Balance>;

		/// Gets the minimum collateral value for the given currency.
		type MinimumCollateralAmount: GetByKey<CurrencyId, Balance>;

		/// Stablecoin currency id
		#[pallet::constant]
		type GetStableCurrencyId: Get<CurrencyId>;

		/// When swap with DEX, the acceptable max slippage for the price from oracle.
		#[pallet::constant]
		type MaxSwapSlippageCompareToOracle: Get<Ratio>;

		/// The CDP treasury to maintain bad debts and surplus generated by CDPs
		type CDPTreasury: CDPTreasuryExtended<Self::AccountId, Balance = Balance, CurrencyId = CurrencyId>;

		/// The price source of all types of currencies related to CDP
		type PriceSource: PriceProvider<CurrencyId>;

		/// A configuration for base priority of unsigned transactions.
		///
		/// This is exposed so that it can be tuned for particular runtime, when
		/// multiple modules send unsigned transactions.
		#[pallet::constant]
		type UnsignedPriority: Get<TransactionPriority>;

		/// Emergency shutdown.
		type EmergencyShutdown: EmergencyShutdown;

		/// Time used for computing era duration.
		///
		/// It is guaranteed to start being called from the first `on_finalize`.
		/// Thus value at genesis is not used.
		type UnixTime: UnixTime;

		/// Currency for transfer assets
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

		/// Dex
		type DEX: DEXManager<Self::AccountId, Balance, CurrencyId>;

		/// Swap
		type Swap: Swap<Self::AccountId, Balance, CurrencyId>;

		/// The origin for liquidation contracts registering and deregistering.
		type LiquidationContractsUpdateOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// When settle collateral with smart contracts, the acceptable max slippage for the price
		/// from oracle.
		#[pallet::constant]
		type MaxLiquidationContractSlippage: Get<Ratio>;

		#[pallet::constant]
		type MaxLiquidationContracts: Get<u32>;

		type LiquidationEvmBridge: LiquidationEvmBridge;

		#[pallet::constant]
		type PalletId: Get<PalletId>;

		type EvmAddressMapping: AddressMapping<Self::AccountId>;

		/// Evm Bridge for getting info of contracts from the EVM.
		type EVMBridge: EVMBridge<Self::AccountId, Balance>;

		/// Evm Origin account when settle erc20 type CDP
		type SettleErc20EvmOrigin: Get<Self::AccountId>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The total debit value of specific collateral type already exceed the
		/// hard cap
		ExceedDebitValueHardCap,
		/// The collateral ratio below the required collateral ratio
		BelowRequiredCollateralRatio,
		/// The collateral ratio below the liquidation ratio
		BelowLiquidationRatio,
		/// The CDP must be unsafe status
		MustBeUnsafe,
		/// The CDP must be safe status
		MustBeSafe,
		/// Invalid collateral type
		InvalidCollateralType,
		/// Remain debit value in CDP below the dust amount
		RemainDebitValueTooSmall,
		/// Remain collateral value in CDP below the dust amount.
		/// Withdraw all collateral or leave more than the minimum.
		CollateralAmountBelowMinimum,
		/// Feed price is invalid
		InvalidFeedPrice,
		/// No debit value in CDP so that it cannot be settled
		NoDebitValue,
		/// System has already been shutdown
		AlreadyShutdown,
		/// Must after system shutdown
		MustAfterShutdown,
		/// Collateral in CDP is not enough
		CollateralNotEnough,
		/// debit value decrement is not enough
		NotEnoughDebitDecrement,
		/// convert debit value to debit balance failed
		ConvertDebitBalanceFailed,
		/// Collateral liquidation failed.
		LiquidationFailed,
		/// Exceeds `T::MaxLiquidationContracts`.
		TooManyLiquidationContracts,
		/// Collateral ERC20 contract not found.
		CollateralContractNotFound,
		/// Invalid rate
		InvalidRate,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Liquidate the unsafe CDP.
		LiquidateUnsafeCDP {
			collateral_type: CurrencyId,
			owner: T::AccountId,
			collateral_amount: Balance,
			bad_debt_value: Balance,
			target_amount: Balance,
		},
		/// Settle the CDP has debit.
		SettleCDPInDebit {
			collateral_type: CurrencyId,
			owner: T::AccountId,
		},
		/// Directly close CDP has debit by handle debit with DEX.
		CloseCDPInDebitByDEX {
			collateral_type: CurrencyId,
			owner: T::AccountId,
			sold_collateral_amount: Balance,
			refund_collateral_amount: Balance,
			debit_value: Balance,
		},
		/// The interest rate per sec for specific collateral type updated.
		InterestRatePerSecUpdated {
			collateral_type: CurrencyId,
			new_interest_rate_per_sec: Option<Rate>,
		},
		/// The liquidation fee for specific collateral type updated.
		LiquidationRatioUpdated {
			collateral_type: CurrencyId,
			new_liquidation_ratio: Option<Ratio>,
		},
		/// The liquidation penalty rate for specific collateral type updated.
		LiquidationPenaltyUpdated {
			collateral_type: CurrencyId,
			new_liquidation_penalty: Option<Rate>,
		},
		/// The required collateral penalty rate for specific collateral type updated.
		RequiredCollateralRatioUpdated {
			collateral_type: CurrencyId,
			new_required_collateral_ratio: Option<Ratio>,
		},
		/// The hard cap of total debit value for specific collateral type updated.
		MaximumTotalDebitValueUpdated {
			collateral_type: CurrencyId,
			new_total_debit_value: Balance,
		},
		/// A new liquidation contract is registered.
		LiquidationContractRegistered { address: EvmAddress },
		/// A new liquidation contract is deregistered.
		LiquidationContractDeregistered { address: EvmAddress },
	}

	/// Mapping from collateral type to its exchange rate of debit units and
	/// debit value
	///
	/// DebitExchangeRate: CurrencyId => Option<ExchangeRate>
	#[pallet::storage]
	#[pallet::getter(fn debit_exchange_rate)]
	pub type DebitExchangeRate<T: Config> = StorageMap<_, Twox64Concat, CurrencyId, ExchangeRate, OptionQuery>;

	/// Mapping from valid collateral type to its risk management params
	///
	/// CollateralParams: CurrencyId => Option<RiskManagementParams>
	#[pallet::storage]
	#[pallet::getter(fn collateral_params)]
	pub type CollateralParams<T: Config> = StorageMap<_, Twox64Concat, CurrencyId, RiskManagementParams, OptionQuery>;

	/// Timestamp in seconds of the last interest accumulation
	///
	/// LastAccumulationSecs: u64
	#[pallet::storage]
	#[pallet::getter(fn last_accumulation_secs)]
	pub type LastAccumulationSecs<T: Config> = StorageValue<_, u64, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn liquidation_contracts)]
	pub type LiquidationContracts<T: Config> =
		StorageValue<_, BoundedVec<EvmAddress, T::MaxLiquidationContracts>, ValueQuery>;

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T> {
		#[allow(clippy::type_complexity)]
		pub collaterals_params: Vec<(
			CurrencyId,
			Option<Rate>,
			Option<Ratio>,
			Option<Rate>,
			Option<Ratio>,
			Balance,
		)>,
		pub _phantom: PhantomData<T>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			self.collaterals_params.iter().for_each(
				|(
					currency_id,
					interest_rate_per_sec,
					liquidation_ratio,
					liquidation_penalty,
					required_collateral_ratio,
					maximum_total_debit_value,
				)| {
					CollateralParams::<T>::insert(
						currency_id,
						RiskManagementParams {
							maximum_total_debit_value: *maximum_total_debit_value,
							interest_rate_per_sec: interest_rate_per_sec
								.map(|v| FractionalRate::try_from(v).expect("interest_rate_per_sec out of bound")),
							liquidation_ratio: *liquidation_ratio,
							liquidation_penalty: liquidation_penalty
								.map(|v| FractionalRate::try_from(v).expect("liquidation_penalty out of bound")),
							required_collateral_ratio: *required_collateral_ratio,
						},
					);
				},
			);
		}
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		/// Issue interest in stable currency for all types of collateral has
		/// debit when block end, and update their debit exchange rate
		fn on_initialize(now: BlockNumberFor<T>) -> Weight {
			// only after the block #1, `T::UnixTime::now()` will not report error.
			// https://github.com/paritytech/substrate/blob/4ff92f10058cfe1b379362673dd369e33a919e66/frame/timestamp/src/lib.rs#L276
			// so accumulate interest at the beginning of the block #2
			let now_as_secs: u64 = if now > One::one() {
				T::UnixTime::now().as_secs()
			} else {
				Default::default()
			};
			<T as Config>::WeightInfo::on_initialize(Self::accumulate_interest(
				now_as_secs,
				Self::last_accumulation_secs(),
			))
		}

		/// Runs after every block. Start offchain worker to check CDP and
		/// submit unsigned tx to trigger liquidation or settlement.
		fn offchain_worker(now: BlockNumberFor<T>) {
			if let Err(e) = Self::_offchain_worker() {
				log::info!(
					target: "cdp-engine offchain worker",
					"cannot run offchain worker at {:?}: {:?}",
					now,
					e,
				);
			} else {
				log::debug!(
					target: "cdp-engine offchain worker",
					"offchain worker start at block: {:?} already done!",
					now,
				);
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Liquidate unsafe CDP
		///
		/// The dispatch origin of this call must be _None_.
		///
		/// - `currency_id`: CDP's collateral type.
		/// - `who`: CDP's owner.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::liquidate_by_auction(<T as Config>::CDPTreasury::max_auction()))]
		pub fn liquidate(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			who: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResultWithPostInfo {
			ensure_none(origin)?;
			let who = T::Lookup::lookup(who)?;
			ensure!(!T::EmergencyShutdown::is_shutdown(), Error::<T>::AlreadyShutdown);
			let consumed_weight: Weight = Self::liquidate_unsafe_cdp(who, currency_id)?;
			Ok(Some(consumed_weight).into())
		}

		/// Settle CDP has debit after system shutdown
		///
		/// The dispatch origin of this call must be _None_.
		///
		/// - `currency_id`: CDP's collateral type.
		/// - `who`: CDP's owner.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::settle())]
		pub fn settle(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			who: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResult {
			ensure_none(origin)?;
			let who = T::Lookup::lookup(who)?;
			ensure!(T::EmergencyShutdown::is_shutdown(), Error::<T>::MustAfterShutdown);
			Self::settle_cdp_has_debit(who, currency_id)?;
			Ok(())
		}

		/// Update parameters related to risk management of CDP under specific
		/// collateral type
		///
		/// The dispatch origin of this call must be `UpdateOrigin`.
		///
		/// - `currency_id`: collateral type.
		/// - `interest_rate_per_sec`: Interest rate per sec, `None` means do not update,
		/// - `liquidation_ratio`: liquidation ratio, `None` means do not update, `Some(None)` means
		///   update it to `None`.
		/// - `liquidation_penalty`: liquidation penalty, `None` means do not update, `Some(None)`
		///   means update it to `None`.
		/// - `required_collateral_ratio`: required collateral ratio, `None` means do not update,
		///   `Some(None)` means update it to `None`.
		/// - `maximum_total_debit_value`: maximum total debit value.
		#[pallet::call_index(2)]
		#[pallet::weight((<T as Config>::WeightInfo::set_collateral_params(), DispatchClass::Operational))]
		pub fn set_collateral_params(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			interest_rate_per_sec: ChangeOptionRate,
			liquidation_ratio: ChangeOptionRatio,
			liquidation_penalty: ChangeOptionRate,
			required_collateral_ratio: ChangeOptionRatio,
			maximum_total_debit_value: ChangeBalance,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;

			let mut collateral_params = Self::collateral_params(currency_id).unwrap_or_default();
			if let Change::NewValue(maybe_rate) = interest_rate_per_sec {
				match (collateral_params.interest_rate_per_sec.as_mut(), maybe_rate) {
					(Some(existing), Some(rate)) => existing.try_set(rate).map_err(|_| Error::<T>::InvalidRate)?,
					(None, Some(rate)) => {
						let fractional_rate = FractionalRate::try_from(rate).map_err(|_| Error::<T>::InvalidRate)?;
						collateral_params.interest_rate_per_sec = Some(fractional_rate);
					}
					_ => collateral_params.interest_rate_per_sec = None,
				}
				Self::deposit_event(Event::InterestRatePerSecUpdated {
					collateral_type: currency_id,
					new_interest_rate_per_sec: maybe_rate,
				});
			}
			if let Change::NewValue(update) = liquidation_ratio {
				collateral_params.liquidation_ratio = update;
				Self::deposit_event(Event::LiquidationRatioUpdated {
					collateral_type: currency_id,
					new_liquidation_ratio: update,
				});
			}
			if let Change::NewValue(maybe_rate) = liquidation_penalty {
				match (collateral_params.liquidation_penalty.as_mut(), maybe_rate) {
					(Some(existing), Some(rate)) => existing.try_set(rate).map_err(|_| Error::<T>::InvalidRate)?,
					(None, Some(rate)) => {
						let fractional_rate = FractionalRate::try_from(rate).map_err(|_| Error::<T>::InvalidRate)?;
						collateral_params.liquidation_penalty = Some(fractional_rate);
					}
					_ => collateral_params.liquidation_penalty = None,
				}
				Self::deposit_event(Event::LiquidationPenaltyUpdated {
					collateral_type: currency_id,
					new_liquidation_penalty: maybe_rate,
				});
			}
			if let Change::NewValue(update) = required_collateral_ratio {
				collateral_params.required_collateral_ratio = update;
				Self::deposit_event(Event::RequiredCollateralRatioUpdated {
					collateral_type: currency_id,
					new_required_collateral_ratio: update,
				});
			}
			if let Change::NewValue(val) = maximum_total_debit_value {
				collateral_params.maximum_total_debit_value = val;
				Self::deposit_event(Event::MaximumTotalDebitValueUpdated {
					collateral_type: currency_id,
					new_total_debit_value: val,
				});
			}
			CollateralParams::<T>::insert(currency_id, collateral_params);
			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::register_liquidation_contract())]
		pub fn register_liquidation_contract(origin: OriginFor<T>, address: EvmAddress) -> DispatchResult {
			T::LiquidationContractsUpdateOrigin::ensure_origin(origin)?;
			LiquidationContracts::<T>::try_append(address).map_err(|()| Error::<T>::TooManyLiquidationContracts)?;
			Self::deposit_event(Event::LiquidationContractRegistered { address });
			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::deregister_liquidation_contract())]
		pub fn deregister_liquidation_contract(origin: OriginFor<T>, address: EvmAddress) -> DispatchResult {
			T::LiquidationContractsUpdateOrigin::ensure_origin(origin)?;
			LiquidationContracts::<T>::mutate(|contracts| {
				contracts.retain(|c| c != &address);
			});
			Self::deposit_event(Event::LiquidationContractDeregistered { address });
			Ok(())
		}
	}

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;

		fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			match call {
				Call::liquidate { currency_id, who } => {
					let account = T::Lookup::lookup(who.clone())?;
					let Position { collateral, debit } = <LoansOf<T>>::positions(currency_id, &account);
					if !matches!(
						Self::check_cdp_status(*currency_id, collateral, debit),
						CDPStatus::Unsafe
					) || T::EmergencyShutdown::is_shutdown()
					{
						return InvalidTransaction::Stale.into();
					}

					ValidTransaction::with_tag_prefix("CDPEngineOffchainWorker")
						.priority(T::UnsignedPriority::get())
						.and_provides((<frame_system::Pallet<T>>::block_number(), currency_id, who))
						.longevity(64_u64)
						.propagate(true)
						.build()
				}
				Call::settle { currency_id, who } => {
					let account = T::Lookup::lookup(who.clone())?;
					let Position { debit, .. } = <LoansOf<T>>::positions(currency_id, account);
					if debit.is_zero() || !T::EmergencyShutdown::is_shutdown() {
						return InvalidTransaction::Stale.into();
					}

					ValidTransaction::with_tag_prefix("CDPEngineOffchainWorker")
						.priority(T::UnsignedPriority::get())
						.and_provides((currency_id, who))
						.longevity(64_u64)
						.propagate(true)
						.build()
				}
				_ => InvalidTransaction::Call.into(),
			}
		}
	}
}

impl<T: Config> Pallet<T> {
	fn accumulate_interest(now_secs: u64, last_accumulation_secs: u64) -> u32 {
		let mut count: u32 = 0;

		if !T::EmergencyShutdown::is_shutdown() && !now_secs.is_zero() {
			let interval_secs = now_secs.saturating_sub(last_accumulation_secs);

			for currency_id in Self::get_collateral_currency_ids() {
				if let Ok(interest_rate) = Self::get_interest_rate_per_sec(currency_id) {
					let rate_to_accumulate = Self::compound_interest_rate(interest_rate, interval_secs);
					let total_debits = <LoansOf<T>>::total_positions(currency_id).debit;

					if !rate_to_accumulate.is_zero() && !total_debits.is_zero() {
						let debit_exchange_rate = Self::get_debit_exchange_rate(currency_id);
						let debit_exchange_rate_increment = debit_exchange_rate.saturating_mul(rate_to_accumulate);
						let issued_stable_coin_balance = debit_exchange_rate_increment.saturating_mul_int(total_debits);

						// issue stablecoin to surplus pool
						let res = <T as Config>::CDPTreasury::on_system_surplus(issued_stable_coin_balance);
						match res {
							Ok(_) => {
								// update exchange rate when issue success
								let new_debit_exchange_rate =
									debit_exchange_rate.saturating_add(debit_exchange_rate_increment);
								DebitExchangeRate::<T>::insert(currency_id, new_debit_exchange_rate);
							}
							Err(e) => {
								log::warn!(
									target: "cdp-engine",
									"on_system_surplus: failed to on system surplus {:?}: {:?}. \
									This is unexpected but should be safe",
									issued_stable_coin_balance, e
								);
							}
						}
					}
					count += 1;
				}
			}
		}

		// update last accumulation timestamp
		LastAccumulationSecs::<T>::put(now_secs);
		count
	}

	fn submit_unsigned_liquidation_tx(currency_id: CurrencyId, who: T::AccountId) {
		let who = T::Lookup::unlookup(who);
		let call = Call::<T>::liquidate {
			currency_id,
			who: who.clone(),
		};
		if SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()).is_err() {
			log::info!(
				target: "cdp-engine offchain worker",
				"submit unsigned liquidation tx for \nCDP - AccountId {:?} CurrencyId {:?} \nfailed!",
				who, currency_id,
			);
		}
	}

	fn submit_unsigned_settlement_tx(currency_id: CurrencyId, who: T::AccountId) {
		let who = T::Lookup::unlookup(who);
		let call = Call::<T>::settle {
			currency_id,
			who: who.clone(),
		};
		if SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()).is_err() {
			log::info!(
				target: "cdp-engine offchain worker",
				"submit unsigned settlement tx for \nCDP - AccountId {:?} CurrencyId {:?} \nfailed!",
				who, currency_id,
			);
		}
	}

	fn _offchain_worker() -> Result<(), OffchainErr> {
		let collateral_currency_ids = Self::get_collateral_currency_ids();
		if collateral_currency_ids.len().is_zero() {
			return Ok(());
		}

		// check if we are a potential validator
		if !sp_io::offchain::is_validator() {
			return Err(OffchainErr::NotValidator);
		}

		// acquire offchain worker lock
		let lock_expiration = Duration::from_millis(LOCK_DURATION);
		let mut lock = StorageLock::<'_, Time>::with_deadline(OFFCHAIN_WORKER_LOCK, lock_expiration);
		let mut guard = lock.try_lock().map_err(|_| OffchainErr::OffchainLock)?;
		let to_be_continue = StorageValueRef::persistent(OFFCHAIN_WORKER_DATA);

		// get to_be_continue record
		let (collateral_position, start_key) =
			if let Ok(Some((last_collateral_position, maybe_last_iterator_previous_key))) =
				to_be_continue.get::<(u32, Option<Vec<u8>>)>()
			{
				(last_collateral_position, maybe_last_iterator_previous_key)
			} else {
				let mut rng = ChaChaRng::from_seed(sp_io::offchain::random_seed());
				(pick_u32(&mut rng, collateral_currency_ids.len() as u32), None)
			};

		// get the max iterations config
		let max_iterations = StorageValueRef::persistent(OFFCHAIN_WORKER_MAX_ITERATIONS)
			.get::<u32>()
			.unwrap_or(Some(DEFAULT_MAX_ITERATIONS))
			.unwrap_or(DEFAULT_MAX_ITERATIONS);

		let currency_id = match collateral_currency_ids.get(collateral_position as usize) {
			Some(currency_id) => *currency_id,
			None => {
				log::debug!(
					target: "cdp-engine offchain worker",
					"collateral_currency was removed, need to reset the offchain worker: collateral_position is {:?}, collateral_currency_ids: {:?}",
					collateral_position,
					collateral_currency_ids
				);
				to_be_continue.set(&(0, Option::<Vec<u8>>::None));
				return Ok(());
			}
		};

		let is_shutdown = T::EmergencyShutdown::is_shutdown();

		// If start key is Some(value) continue iterating from that point in storage otherwise start
		// iterating from the beginning of <module_loans::Positions<T>>
		let mut map_iterator = match start_key.clone() {
			Some(key) => <module_loans::Positions<T>>::iter_prefix_from(currency_id, key),
			None => <module_loans::Positions<T>>::iter_prefix(currency_id),
		};

		let mut finished = true;
		let mut iteration_count = 0;
		let iteration_start_time = sp_io::offchain::timestamp();

		#[allow(clippy::while_let_on_iterator)]
		while let Some((who, Position { collateral, debit })) = map_iterator.next() {
			if !is_shutdown
				&& matches!(
					Self::check_cdp_status(currency_id, collateral, debit),
					CDPStatus::Unsafe
				) {
				// liquidate unsafe CDPs before emergency shutdown occurs
				Self::submit_unsigned_liquidation_tx(currency_id, who);
			} else if is_shutdown && !debit.is_zero() {
				// settle CDPs with debit after emergency shutdown occurs.
				Self::submit_unsigned_settlement_tx(currency_id, who);
			}

			iteration_count += 1;
			if iteration_count == max_iterations {
				finished = false;
				break;
			}
			// extend offchain worker lock
			guard.extend_lock().map_err(|_| OffchainErr::OffchainLock)?;
		}
		let iteration_end_time = sp_io::offchain::timestamp();
		log::debug!(
			target: "cdp-engine offchain worker",
			"iteration info:\n max iterations is {:?}\n currency id: {:?}, start key: {:?}, iterate count: {:?}\n iteration start at: {:?}, end at: {:?}, execution time: {:?}\n",
			max_iterations,
			currency_id,
			start_key,
			iteration_count,
			iteration_start_time,
			iteration_end_time,
			iteration_end_time.diff(&iteration_start_time)
		);

		// if iteration for map storage finished, clear to be continue record
		// otherwise, update to be continue record
		if finished {
			let next_collateral_position =
				if collateral_position < collateral_currency_ids.len().saturating_sub(1) as u32 {
					collateral_position + 1
				} else {
					0
				};
			to_be_continue.set(&(next_collateral_position, Option::<Vec<u8>>::None));
		} else {
			to_be_continue.set(&(collateral_position, Some(map_iterator.last_raw_key())));
		}

		// Consume the guard but **do not** unlock the underlying lock.
		guard.forget();

		Ok(())
	}

	pub fn check_cdp_status(currency_id: CurrencyId, collateral_amount: Balance, debit_amount: Balance) -> CDPStatus {
		let stable_currency_id = T::GetStableCurrencyId::get();
		if let Some(feed_price) = T::PriceSource::get_relative_price(currency_id, stable_currency_id) {
			let collateral_ratio =
				Self::calculate_collateral_ratio(currency_id, collateral_amount, debit_amount, feed_price);
			match Self::get_liquidation_ratio(currency_id) {
				Ok(liquidation_ratio) => {
					if collateral_ratio < liquidation_ratio {
						CDPStatus::Unsafe
					} else {
						CDPStatus::Safe
					}
				}
				Err(e) => CDPStatus::ChecksFailed(e),
			}
		} else {
			CDPStatus::ChecksFailed(Error::<T>::InvalidFeedPrice.into())
		}
	}

	pub fn maximum_total_debit_value(currency_id: CurrencyId) -> Result<Balance, DispatchError> {
		let params = Self::collateral_params(currency_id).ok_or(Error::<T>::InvalidCollateralType)?;
		Ok(params.maximum_total_debit_value)
	}

	pub fn required_collateral_ratio(currency_id: CurrencyId) -> Result<Option<Ratio>, DispatchError> {
		let params = Self::collateral_params(currency_id).ok_or(Error::<T>::InvalidCollateralType)?;
		Ok(params.required_collateral_ratio)
	}

	pub fn get_interest_rate_per_sec(currency_id: CurrencyId) -> Result<Rate, DispatchError> {
		let params = Self::collateral_params(currency_id).ok_or(Error::<T>::InvalidCollateralType)?;
		params
			.interest_rate_per_sec
			.map(|v| v.into_inner())
			.ok_or_else(|| Error::<T>::InvalidCollateralType.into())
	}

	pub fn compound_interest_rate(rate_per_sec: Rate, secs: u64) -> Rate {
		rate_per_sec
			.saturating_add(Rate::one())
			.saturating_pow(secs.unique_saturated_into())
			.saturating_sub(Rate::one())
	}

	pub fn get_liquidation_ratio(currency_id: CurrencyId) -> Result<Ratio, DispatchError> {
		let params = Self::collateral_params(currency_id).ok_or(Error::<T>::InvalidCollateralType)?;
		Ok(params.liquidation_ratio.unwrap_or_else(T::DefaultLiquidationRatio::get))
	}

	pub fn get_liquidation_penalty(currency_id: CurrencyId) -> Result<Rate, DispatchError> {
		let params = Self::collateral_params(currency_id).ok_or(Error::<T>::InvalidCollateralType)?;
		Ok(params
			.liquidation_penalty
			.map(|v| v.into_inner())
			.unwrap_or_else(|| T::DefaultLiquidationPenalty::get().into_inner()))
	}

	pub fn get_debit_exchange_rate(currency_id: CurrencyId) -> ExchangeRate {
		Self::debit_exchange_rate(currency_id).unwrap_or_else(T::DefaultDebitExchangeRate::get)
	}

	pub fn convert_to_debit_value(currency_id: CurrencyId, debit_balance: Balance) -> Balance {
		Self::get_debit_exchange_rate(currency_id).saturating_mul_int(debit_balance)
	}

	pub fn try_convert_to_debit_balance(currency_id: CurrencyId, debit_value: Balance) -> Option<Balance> {
		Self::get_debit_exchange_rate(currency_id)
			.reciprocal()
			.map(|n| n.saturating_mul_int(debit_value))
	}

	pub fn calculate_collateral_ratio(
		currency_id: CurrencyId,
		collateral_balance: Balance,
		debit_balance: Balance,
		price: Price,
	) -> Ratio {
		let locked_collateral_value = price.saturating_mul_int(collateral_balance);
		let debit_value = Self::get_debit_value(currency_id, debit_balance);

		Ratio::checked_from_rational(locked_collateral_value, debit_value).unwrap_or_else(Ratio::max_value)
	}

	pub fn adjust_position(
		who: &T::AccountId,
		currency_id: CurrencyId,
		collateral_adjustment: Amount,
		debit_adjustment: Amount,
	) -> DispatchResult {
		ensure!(
			CollateralParams::<T>::contains_key(currency_id),
			Error::<T>::InvalidCollateralType,
		);
		<LoansOf<T>>::adjust_position(who, currency_id, collateral_adjustment, debit_adjustment)?;
		Ok(())
	}

	pub fn adjust_position_by_debit_value(
		who: &T::AccountId,
		currency_id: CurrencyId,
		collateral_adjustment: Amount,
		debit_value_adjustment: Amount,
	) -> DispatchResult {
		let debit_value_adjustment_abs = <LoansOf<T>>::balance_try_from_amount_abs(debit_value_adjustment)?;
		let debit_adjustment_abs = Self::try_convert_to_debit_balance(currency_id, debit_value_adjustment_abs)
			.ok_or(Error::<T>::ConvertDebitBalanceFailed)?;

		if debit_value_adjustment.is_negative() {
			let Position { collateral: _, debit } = <LoansOf<T>>::positions(currency_id, who);
			let actual_adjustment_abs = debit.min(debit_adjustment_abs);
			let debit_adjustment = <LoansOf<T>>::amount_try_from_balance(actual_adjustment_abs)?;

			Self::adjust_position(
				who,
				currency_id,
				collateral_adjustment,
				debit_adjustment.saturating_neg(),
			)?;
		} else {
			let debit_adjustment = <LoansOf<T>>::amount_try_from_balance(debit_adjustment_abs)?;
			Self::adjust_position(who, currency_id, collateral_adjustment, debit_adjustment)?;
		}

		Ok(())
	}

	/// If reverse is false, swap stable coin to given `token`.
	/// If reverse is true, swap given `token` to stable coin.
	fn swap_stable_and_lp_token(
		token: CurrencyId,
		amount: Balance,
		reverse: bool,
	) -> sp_std::result::Result<Balance, DispatchError> {
		let stable_currency_id = T::GetStableCurrencyId::get();
		let loans_module_account = <LoansOf<T>>::account_id();

		// do nothing if given token is stable coin
		if token == stable_currency_id {
			return Ok(amount);
		}

		let (supply, target) = if reverse {
			(token, stable_currency_id)
		} else {
			(stable_currency_id, token)
		};

		T::Swap::swap(
			&loans_module_account,
			supply,
			target,
			SwapLimit::ExactSupply(amount, Zero::zero()),
		)
		.map(|e| e.1)
	}

	/// Generate new debit in advance, buy collateral and deposit it into CDP,
	/// and the collateral ratio will be reduced but CDP must still be at valid risk.
	/// For single token collateral, try to swap collateral by DEX. For lp token collateral,
	/// try to swap lp components by DEX first, then add liquidity to obtain lp token,
	/// CDP owner may receive some remainer assets.
	#[transactional]
	pub fn expand_position_collateral(
		who: &T::AccountId,
		currency_id: CurrencyId,
		increase_debit_value: Balance,
		min_increase_collateral: Balance,
	) -> DispatchResult {
		ensure!(
			CollateralParams::<T>::contains_key(currency_id),
			Error::<T>::InvalidCollateralType,
		);
		let loans_module_account = <LoansOf<T>>::account_id();

		// issue stable coin in advance
		<T as Config>::CDPTreasury::issue_debit(&loans_module_account, increase_debit_value, true)?;

		// get the actual increased collateral amount
		let increase_collateral = match currency_id {
			CurrencyId::DexShare(dex_share_0, dex_share_1) => {
				let token_0: CurrencyId = dex_share_0.into();
				let token_1: CurrencyId = dex_share_1.into();

				// NOTE: distribute half of the new issued stable coin to each components of lp token,
				// need better distribution methods to avoid unused component tokens.
				let stable_for_token_0 = increase_debit_value / 2;
				let stable_for_token_1 = increase_debit_value.saturating_sub(stable_for_token_0);

				// swap stable coin to lp component tokens.
				let available_0 = Self::swap_stable_and_lp_token(token_0, stable_for_token_0, false)?;
				let available_1 = Self::swap_stable_and_lp_token(token_1, stable_for_token_1, false)?;
				let (consumption_0, consumption_1, actual_increase_lp) = T::DEX::add_liquidity(
					&loans_module_account,
					token_0,
					token_1,
					available_0,
					available_1,
					min_increase_collateral,
					false,
				)?;

				// refund unused lp component tokens
				if let Some(remainer) = available_0.checked_sub(consumption_0) {
					<T as Config>::Currency::transfer(token_0, &loans_module_account, who, remainer)?;
				}
				if let Some(remainer) = available_1.checked_sub(consumption_1) {
					<T as Config>::Currency::transfer(token_1, &loans_module_account, who, remainer)?;
				}

				actual_increase_lp
			}
			_ => {
				// swap stable coin to collateral
				let limit = SwapLimit::ExactSupply(increase_debit_value, min_increase_collateral);
				let (_, actual_increase_collateral) =
					T::Swap::swap(&loans_module_account, T::GetStableCurrencyId::get(), currency_id, limit)?;

				actual_increase_collateral
			}
		};

		// update CDP state
		let collateral_adjustment = <LoansOf<T>>::amount_try_from_balance(increase_collateral)?;
		let increase_debit_balance = Self::try_convert_to_debit_balance(currency_id, increase_debit_value)
			.ok_or(Error::<T>::ConvertDebitBalanceFailed)?;
		let debit_adjustment = <LoansOf<T>>::amount_try_from_balance(increase_debit_balance)?;
		<LoansOf<T>>::update_loan(who, currency_id, collateral_adjustment, debit_adjustment)?;

		let Position { collateral, debit } = <LoansOf<T>>::positions(currency_id, who);
		// check the CDP if is still at valid risk
		Self::check_position_valid(currency_id, collateral, debit, false)?;
		// debit cap check due to new issued stable coin
		Self::check_debit_cap(currency_id, <LoansOf<T>>::total_positions(currency_id).debit)?;
		Ok(())
	}

	/// Sell the collateral locked in CDP to get stable coin to repay the debit,
	/// and the collateral ratio will be increased. For single token collateral,
	/// try to swap stable coin by DEX. For lp token collateral, try to remove liquidity
	/// for lp token first, then swap the non-stable coin to get stable coin. If all
	/// debit are repaid, the extra stable coin will be transferred back to the CDP
	/// owner directly.
	#[transactional]
	pub fn shrink_position_debit(
		who: &T::AccountId,
		currency_id: CurrencyId,
		decrease_collateral: Balance,
		min_decrease_debit_value: Balance,
	) -> DispatchResult {
		ensure!(
			CollateralParams::<T>::contains_key(currency_id),
			Error::<T>::InvalidCollateralType,
		);

		let loans_module_account = <LoansOf<T>>::account_id();
		let stable_currency_id = T::GetStableCurrencyId::get();
		let Position { collateral, debit } = <LoansOf<T>>::positions(currency_id, who);

		// ensure collateral of CDP is enough
		ensure!(decrease_collateral <= collateral, Error::<T>::CollateralNotEnough);

		let actual_stable_amount = match currency_id {
			CurrencyId::DexShare(dex_share_0, dex_share_1) => {
				let token_0: CurrencyId = dex_share_0.into();
				let token_1: CurrencyId = dex_share_1.into();

				// remove liquidity to get component tokens of lp token
				let (available_0, available_1) = T::DEX::remove_liquidity(
					&loans_module_account,
					token_0,
					token_1,
					decrease_collateral,
					Zero::zero(),
					Zero::zero(),
					false,
				)?;

				let stable_0 = Self::swap_stable_and_lp_token(token_0, available_0, true)?;
				let stable_1 = Self::swap_stable_and_lp_token(token_1, available_1, true)?;
				let total_stable = stable_0.saturating_add(stable_1);

				// check whether the amount of stable token obtained by selling lptokens is enough as expected
				ensure!(
					total_stable >= min_decrease_debit_value,
					Error::<T>::NotEnoughDebitDecrement
				);

				total_stable
			}
			_ => {
				// swap collateral to stable coin
				let limit = SwapLimit::ExactSupply(decrease_collateral, min_decrease_debit_value);
				let (_, actual_stable) = T::Swap::swap(&loans_module_account, currency_id, stable_currency_id, limit)?;

				actual_stable
			}
		};

		// update CDP state
		let collateral_adjustment = <LoansOf<T>>::amount_try_from_balance(decrease_collateral)?.saturating_neg();
		let previous_debit_value = Self::get_debit_value(currency_id, debit);
		let (decrease_debit_value, decrease_debit_balance) = if actual_stable_amount >= previous_debit_value {
			// refund extra stable coin to the CDP owner
			<T as Config>::Currency::transfer(
				stable_currency_id,
				&loans_module_account,
				who,
				actual_stable_amount.saturating_sub(previous_debit_value),
			)?;

			(previous_debit_value, debit)
		} else {
			(
				actual_stable_amount,
				Self::try_convert_to_debit_balance(currency_id, actual_stable_amount)
					.ok_or(Error::<T>::ConvertDebitBalanceFailed)?,
			)
		};

		let debit_adjustment = <LoansOf<T>>::amount_try_from_balance(decrease_debit_balance)?.saturating_neg();
		<LoansOf<T>>::update_loan(who, currency_id, collateral_adjustment, debit_adjustment)?;

		// repay the debit of CDP
		<T as Config>::CDPTreasury::burn_debit(&loans_module_account, decrease_debit_value)?;

		// check the CDP if is still at valid risk.
		Self::check_position_valid(
			currency_id,
			collateral.saturating_sub(decrease_collateral),
			debit.saturating_sub(decrease_debit_balance),
			false,
		)?;
		Ok(())
	}

	// settle cdp has debit when emergency shutdown
	pub fn settle_cdp_has_debit(who: T::AccountId, currency_id: CurrencyId) -> DispatchResult {
		let Position { collateral, debit } = <LoansOf<T>>::positions(currency_id, &who);
		ensure!(!debit.is_zero(), Error::<T>::NoDebitValue);

		// confiscate collateral in cdp to cdp treasury
		// and decrease CDP's debit to zero
		let settle_price: Price = T::PriceSource::get_relative_price(T::GetStableCurrencyId::get(), currency_id)
			.ok_or(Error::<T>::InvalidFeedPrice)?;
		let bad_debt_value = Self::get_debit_value(currency_id, debit);
		let confiscate_collateral_amount =
			sp_std::cmp::min(settle_price.saturating_mul_int(bad_debt_value), collateral);

		if let CurrencyId::Erc20(_) = currency_id {
			T::EVMBridge::set_origin(T::SettleErc20EvmOrigin::get());
		}

		// confiscate collateral and all debit
		<LoansOf<T>>::confiscate_collateral_and_debit(&who, currency_id, confiscate_collateral_amount, debit)?;

		if let CurrencyId::Erc20(_) = currency_id {
			T::EVMBridge::kill_origin();
		}

		Self::deposit_event(Event::SettleCDPInDebit {
			collateral_type: currency_id,
			owner: who,
		});
		Ok(())
	}

	// close cdp has debit by swap collateral to exact debit
	#[transactional]
	pub fn close_cdp_has_debit_by_dex(
		who: T::AccountId,
		currency_id: CurrencyId,
		max_collateral_amount: Balance,
	) -> DispatchResult {
		let Position { collateral, debit } = <LoansOf<T>>::positions(currency_id, &who);
		ensure!(!debit.is_zero(), Error::<T>::NoDebitValue);
		ensure!(
			matches!(Self::check_cdp_status(currency_id, collateral, debit), CDPStatus::Safe),
			Error::<T>::MustBeSafe
		);

		// confiscate all collateral and debit of unsafe cdp to cdp treasury
		<LoansOf<T>>::confiscate_collateral_and_debit(&who, currency_id, collateral, debit)?;

		// swap exact stable with DEX in limit of price impact
		let debit_value = Self::get_debit_value(currency_id, debit);
		let collateral_supply = collateral.min(max_collateral_amount);

		let (actual_supply_collateral, _) = <T as Config>::CDPTreasury::swap_collateral_to_stable(
			currency_id,
			SwapLimit::ExactTarget(collateral_supply, debit_value),
			false,
		)?;

		// refund remain collateral to CDP owner
		let refund_collateral_amount = collateral
			.checked_sub(actual_supply_collateral)
			.expect("swap success means collateral >= actual_supply_collateral; qed");
		<T as Config>::CDPTreasury::withdraw_collateral(&who, currency_id, refund_collateral_amount)?;

		Self::deposit_event(Event::CloseCDPInDebitByDEX {
			collateral_type: currency_id,
			owner: who,
			sold_collateral_amount: actual_supply_collateral,
			refund_collateral_amount,
			debit_value,
		});
		Ok(())
	}

	// liquidate unsafe cdp
	pub fn liquidate_unsafe_cdp(who: T::AccountId, currency_id: CurrencyId) -> Result<Weight, DispatchError> {
		let Position { collateral, debit } = <LoansOf<T>>::positions(currency_id, &who);

		// ensure the cdp is unsafe
		ensure!(
			matches!(
				Self::check_cdp_status(currency_id, collateral, debit),
				CDPStatus::Unsafe
			),
			Error::<T>::MustBeUnsafe
		);

		// confiscate all collateral and debit of unsafe cdp to cdp treasury
		<LoansOf<T>>::confiscate_collateral_and_debit(&who, currency_id, collateral, debit)?;

		let bad_debt_value = Self::get_debit_value(currency_id, debit);
		let liquidation_penalty = Self::get_liquidation_penalty(currency_id)?;
		let target_stable_amount = liquidation_penalty.saturating_mul_acc_int(bad_debt_value);

		match currency_id {
			CurrencyId::DexShare(dex_share_0, dex_share_1) => {
				let token_0: CurrencyId = dex_share_0.into();
				let token_1: CurrencyId = dex_share_1.into();

				// remove liquidity first
				let (amount_0, amount_1) =
					<T as Config>::CDPTreasury::remove_liquidity_for_lp_collateral(currency_id, collateral)?;

				// if these's stable
				let stable_currency_id = T::GetStableCurrencyId::get();
				if token_0 == stable_currency_id || token_1 == stable_currency_id {
					let (existing_stable, need_handle_currency, handle_amount) = if token_0 == stable_currency_id {
						(amount_0, token_1, amount_1)
					} else {
						(amount_1, token_0, amount_0)
					};

					// these's stable refund
					if existing_stable > target_stable_amount {
						<T as Config>::CDPTreasury::withdraw_collateral(
							&who,
							stable_currency_id,
							existing_stable
								.checked_sub(target_stable_amount)
								.expect("ensured existing stable amount greater than target; qed"),
						)?;
					}

					let remain_target = target_stable_amount.saturating_sub(existing_stable);
					Self::handle_liquidated_collateral(&who, need_handle_currency, handle_amount, remain_target)?;
				} else {
					// token_0 and token_1 each take half target_stable
					let target_0 = target_stable_amount / 2;
					let target_1 = target_stable_amount.saturating_sub(target_0);
					Self::handle_liquidated_collateral(&who, token_0, amount_0, target_0)?;
					Self::handle_liquidated_collateral(&who, token_1, amount_1, target_1)?;
				}
			}
			_ => {
				Self::handle_liquidated_collateral(&who, currency_id, collateral, target_stable_amount)?;
			}
		}

		Self::deposit_event(Event::LiquidateUnsafeCDP {
			collateral_type: currency_id,
			owner: who,
			collateral_amount: collateral,
			bad_debt_value,
			target_amount: target_stable_amount,
		});
		Ok(T::WeightInfo::liquidate_by_dex())
	}

	pub fn handle_liquidated_collateral(
		who: &T::AccountId,
		currency_id: CurrencyId,
		amount: Balance,
		target_stable_amount: Balance,
	) -> DispatchResult {
		if target_stable_amount.is_zero() {
			// refund collateral to CDP owner
			if !amount.is_zero() {
				<T as Config>::CDPTreasury::withdraw_collateral(who, currency_id, amount)?;
			}
			return Ok(());
		}
		LiquidateByPriority::<T>::liquidate(who, currency_id, amount, target_stable_amount)
	}

	pub fn get_collateral_currency_ids() -> Vec<CurrencyId> {
		CollateralParams::<T>::iter_keys().collect()
	}

	fn account_id() -> T::AccountId {
		<T as Config>::PalletId::get().into_account_truncating()
	}

	/// Pallet EVM address, derived from pallet id.
	fn evm_address() -> EvmAddress {
		T::EvmAddressMapping::get_or_create_evm_address(&Self::account_id())
	}
}

type LiquidateByPriority<T> = (LiquidateViaDex<T>, LiquidateViaContracts<T>, LiquidateViaAuction<T>);

pub struct LiquidateViaDex<T>(PhantomData<T>);
impl<T: Config> LiquidateCollateral<T::AccountId> for LiquidateViaDex<T> {
	fn liquidate(
		who: &T::AccountId,
		currency_id: CurrencyId,
		amount: Balance,
		target_stable_amount: Balance,
	) -> DispatchResult {
		// calculate the supply limit by slippage limit for the price of oracle,
		let max_supply_limit = Ratio::one()
			.saturating_sub(T::MaxSwapSlippageCompareToOracle::get())
			.reciprocal()
			.unwrap_or_else(Ratio::max_value)
			.saturating_mul_int(
				T::PriceSource::get_relative_price(T::GetStableCurrencyId::get(), currency_id)
					.expect("the oracle price should be available because liquidation are triggered by it.")
					.saturating_mul_int(target_stable_amount),
			);
		let collateral_supply = amount.min(max_supply_limit);

		let (actual_supply_collateral, actual_target_amount) = <T as Config>::CDPTreasury::swap_collateral_to_stable(
			currency_id,
			SwapLimit::ExactTarget(collateral_supply, target_stable_amount),
			false,
		)?;

		let refund_collateral_amount = amount
			.checked_sub(actual_supply_collateral)
			.expect("swap success means collateral >= actual_supply_collateral; qed");
		// refund remain collateral to CDP owner
		if !refund_collateral_amount.is_zero() {
			<T as Config>::CDPTreasury::withdraw_collateral(who, currency_id, refund_collateral_amount)?;
		}

		// Note: for StableAsset, the swap of cdp treasury is always on `ExactSupply`
		// regardless of this swap_limit params. There will be excess stablecoins that
		// need to be returned to the `who` from cdp treasury account.
		if actual_target_amount > target_stable_amount {
			<T as Config>::CDPTreasury::withdraw_surplus(
				who,
				actual_target_amount.saturating_sub(target_stable_amount),
			)?;
		}

		Ok(())
	}
}

pub struct LiquidateViaContracts<T>(PhantomData<T>);
impl<T: Config> LiquidateCollateral<T::AccountId> for LiquidateViaContracts<T> {
	fn liquidate(
		who: &T::AccountId,
		currency_id: CurrencyId,
		amount: Balance,
		target_stable_amount: Balance,
	) -> DispatchResult {
		let liquidation_contracts = Pallet::<T>::liquidation_contracts();
		let liquidation_contracts_len = liquidation_contracts.len();
		if liquidation_contracts_len.is_zero() {
			return Err(Error::<T>::LiquidationFailed.into());
		}

		let max_supply_limit = Ratio::one()
			.saturating_sub(T::MaxLiquidationContractSlippage::get())
			.reciprocal()
			.unwrap_or_else(Ratio::max_value)
			.saturating_mul_int(
				T::PriceSource::get_relative_price(T::GetStableCurrencyId::get(), currency_id)
					.expect("the oracle price should be available because liquidation are triggered by it.")
					.saturating_mul_int(target_stable_amount),
			);
		let collateral_supply = amount.min(max_supply_limit);

		let collateral = currency_id
			.erc20_address()
			.ok_or(Error::<T>::CollateralContractNotFound)?;
		let repay_dest = Pallet::<T>::evm_address();
		let repay_dest_account_id = Pallet::<T>::account_id();

		let stable_coin = T::GetStableCurrencyId::get();

		let contracts_by_priority = {
			let now: usize = frame_system::Pallet::<T>::current_block_number()
				.try_into()
				.map_err(|_| ArithmeticError::Overflow)?;
			// can't fail as ensured `liquidation_contracts_len` non-zero
			let start_at = now % liquidation_contracts_len;
			let mut all: Vec<EvmAddress> = liquidation_contracts.into();
			let mut right = all.split_off(start_at);
			right.append(&mut all);
			right
		};

		// try liquidation on each contract
		for contract in contracts_by_priority.into_iter() {
			let repay_dest_balance = CurrencyOf::<T>::free_balance(stable_coin, &repay_dest_account_id);
			if T::LiquidationEvmBridge::liquidate(
				InvokeContext {
					contract,
					sender: repay_dest,
					origin: contract,
				},
				collateral,
				repay_dest,
				collateral_supply,
				target_stable_amount,
			)
			.is_ok()
			{
				let repayment = CurrencyOf::<T>::free_balance(stable_coin, &repay_dest_account_id)
					.saturating_sub(repay_dest_balance);
				let contract_account_id = T::EvmAddressMapping::get_account_id(&contract);
				if repayment >= target_stable_amount {
					// sufficient repayment, transfer collateral to contract and notify
					if let Err(e) = <T as Config>::CDPTreasury::withdraw_collateral(
						&contract_account_id,
						currency_id,
						collateral_supply,
					) {
						log::error!(
							target: "cdp-engine",
							"LiquidateViaContracts: transfer collateral to contract failed. \
							Collateral: {:?}, amount: {:?} contract: {:?}, error: {:?}. \
							This is unexpected, need extra action.",
							currency_id, collateral_supply, contract, e,
						);
					} else {
						// notify liquidation success
						T::LiquidationEvmBridge::on_collateral_transfer(
							InvokeContext {
								contract,
								sender: repay_dest,
								origin: contract,
							},
							collateral,
							target_stable_amount,
						);
					}
					// refund rest collateral to CDP owner
					let refund_collateral_amount = amount
						.checked_sub(collateral_supply)
						.expect("Ensured collateral supply <= amount; qed");
					if !refund_collateral_amount.is_zero() {
						if let Err(e) =
							<T as Config>::CDPTreasury::withdraw_collateral(who, currency_id, refund_collateral_amount)
						{
							log::error!(
								target: "cdp-engine",
								"LiquidateViaContracts: refund rest collateral to CDP owner failed. \
								Collateral: {:?}, amount: {:?} error: {:?}. \
								This is unexpected, need extra action.",
								currency_id, refund_collateral_amount, e,
							);
						}
					}
					return Ok(());
				} else if repayment > 0 {
					// insufficient repayment, refund
					CurrencyOf::<T>::transfer(stable_coin, &repay_dest_account_id, &contract_account_id, repayment)?;
					// notify liquidation failed
					T::LiquidationEvmBridge::on_repayment_refund(
						InvokeContext {
							contract,
							sender: Pallet::<T>::evm_address(),
							origin: contract,
						},
						collateral,
						repayment,
					);
				}
			}
		}

		Err(Error::<T>::LiquidationFailed.into())
	}
}

pub struct LiquidateViaAuction<T>(PhantomData<T>);
impl<T: Config> LiquidateCollateral<T::AccountId> for LiquidateViaAuction<T> {
	fn liquidate(
		who: &T::AccountId,
		currency_id: CurrencyId,
		amount: Balance,
		target_stable_amount: Balance,
	) -> DispatchResult {
		<T as Config>::CDPTreasury::create_collateral_auctions(
			currency_id,
			amount,
			target_stable_amount,
			who.clone(),
			true,
		)
		.map(|_| ())
	}
}

impl<T: Config> RiskManager<T::AccountId, CurrencyId, Balance, Balance> for Pallet<T> {
	fn get_debit_value(currency_id: CurrencyId, debit_balance: Balance) -> Balance {
		Self::convert_to_debit_value(currency_id, debit_balance)
	}

	fn check_position_valid(
		currency_id: CurrencyId,
		collateral_balance: Balance,
		debit_balance: Balance,
		check_required_ratio: bool,
	) -> DispatchResult {
		if !debit_balance.is_zero() {
			let debit_value = Self::get_debit_value(currency_id, debit_balance);
			let feed_price = <T as Config>::PriceSource::get_relative_price(currency_id, T::GetStableCurrencyId::get())
				.ok_or(Error::<T>::InvalidFeedPrice)?;
			let collateral_ratio =
				Self::calculate_collateral_ratio(currency_id, collateral_balance, debit_balance, feed_price);

			// check the required collateral ratio
			if check_required_ratio {
				if let Some(required_collateral_ratio) = Self::required_collateral_ratio(currency_id)? {
					ensure!(
						collateral_ratio >= required_collateral_ratio,
						Error::<T>::BelowRequiredCollateralRatio
					);
				}
			}

			// check the liquidation ratio
			let liquidation_ratio = Self::get_liquidation_ratio(currency_id)?;
			ensure!(collateral_ratio >= liquidation_ratio, Error::<T>::BelowLiquidationRatio);

			// check the minimum_debit_value
			ensure!(
				debit_value >= T::MinimumDebitValue::get(),
				Error::<T>::RemainDebitValueTooSmall,
			);
		} else if !collateral_balance.is_zero() {
			// If there are any collateral remaining, then it must be above the minimum
			ensure!(
				collateral_balance >= T::MinimumCollateralAmount::get(&currency_id),
				Error::<T>::CollateralAmountBelowMinimum,
			);
		}

		Ok(())
	}

	fn check_debit_cap(currency_id: CurrencyId, total_debit_balance: Balance) -> DispatchResult {
		let hard_cap = Self::maximum_total_debit_value(currency_id)?;
		let total_debit_value = Self::get_debit_value(currency_id, total_debit_balance);

		ensure!(total_debit_value <= hard_cap, Error::<T>::ExceedDebitValueHardCap);

		Ok(())
	}
}

pub struct CollateralCurrencyIds<T>(PhantomData<T>);
// Returns a list of currently supported/configured collateral currency
impl<T: Config> Get<Vec<CurrencyId>> for CollateralCurrencyIds<T> {
	fn get() -> Vec<CurrencyId> {
		Pallet::<T>::get_collateral_currency_ids()
	}
}

/// Pick a new PRN, in the range [0, `max`) (exclusive).
fn pick_u32<R: RngCore>(rng: &mut R, max: u32) -> u32 {
	rng.next_u32() % max
}
