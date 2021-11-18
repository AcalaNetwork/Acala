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

mod mock_no_fees;
mod tests_no_fees;
pub mod weights;

use frame_support::{log, pallet_prelude::*, transactional, weights::Weight, BoundedVec};
use frame_system::{ensure_signed, pallet_prelude::*};

use module_support::{CallBuilder, ExchangeRate, ExchangeRateProvider, Ratio};
use orml_traits::{
	arithmetic::Signed, BalanceStatus, MultiCurrency, MultiCurrencyExtended, MultiReservableCurrency, XcmTransfer,
};
use primitives::{Balance, CurrencyId};
use sp_arithmetic::traits::CheckedRem;
use sp_runtime::{
	traits::{BlockNumberProvider, Bounded, Saturating, Zero},
	ArithmeticError, FixedPointNumber, Permill,
};
use sp_std::{
	cmp::{min, Ordering},
	convert::{From, TryFrom, TryInto},
	ops::Mul,
	prelude::*,
};
use xcm::latest::prelude::*;

pub use module::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod module {
	use super::*;

	pub type RelayChainBlockNumberOf<T> = <<T as Config>::RelayChainBlockNumber as BlockNumberProvider>::BlockNumber;
	pub(crate) type AmountOf<T> =
		<<T as Config>::Currency as MultiCurrencyExtended<<T as frame_system::Config>::AccountId>>::Amount;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_xcm::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;

		/// Multi-currency support for asset management
		type Currency: MultiReservableCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>
			+ MultiCurrencyExtended<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

		/// The Currency ID for the Staking asset
		#[pallet::constant]
		type StakingCurrencyId: Get<CurrencyId>;

		/// The Currency ID for the Liquid asset
		#[pallet::constant]
		type LiquidCurrencyId: Get<CurrencyId>;

		/// Origin represented Governance
		type GovernanceOrigin: EnsureOrigin<<Self as frame_system::Config>::Origin>;

		/// The minimal amount of Staking currency to be locked
		#[pallet::constant]
		type MinimumMintThreshold: Get<Balance>;

		/// The minimal amount of Liquid currency to be Redeemed
		#[pallet::constant]
		type MinimumRedeemThreshold: Get<Balance>;

		/// The interface to Cross-chain transfer.
		type XcmTransfer: XcmTransfer<Self::AccountId, Balance, CurrencyId>;

		/// The Call builder for communicating with RelayChain via XCM messaging.
		type RelayChainCallBuilder: CallBuilder<AccountId = Self::AccountId, Balance = Balance>;

		/// The MultiLocation of the sovereign sub-account for where the staking currencies are sent
		/// to.
		#[pallet::constant]
		type SovereignSubAccountLocation: Get<MultiLocation>;

		/// The Index to the Homa Lite Sub-account
		#[pallet::constant]
		type SubAccountIndex: Get<u16>;

		/// The default exchange rate for liquid currency to staking currency.
		#[pallet::constant]
		type DefaultExchangeRate: Get<ExchangeRate>;

		/// The maximum rewards that are earned on the relaychain.
		#[pallet::constant]
		type MaxRewardPerEra: Get<Permill>;

		/// The fixed cost of transaction fee for XCM transfers.
		#[pallet::constant]
		type MintFee: Get<Balance>;

		/// Equivalent to the loss of % staking reward from unbonding on the RelayChain.
		#[pallet::constant]
		type BaseWithdrawFee: Get<Permill>;

		/// The fixed cost of withdrawing Staking currency via redeem. In Staking currency.
		#[pallet::constant]
		type XcmUnbondFee: Get<Balance>;

		/// Block number provider for the relaychain.
		type RelayChainBlockNumber: BlockNumberProvider<BlockNumber = Self::BlockNumber>;

		/// The account ID to redeem from on the relaychain.
		#[pallet::constant]
		type ParachainAccount: Get<Self::AccountId>;

		/// The maximum number of redeem requests to match in "Mint" extrinsic.
		#[pallet::constant]
		type MaximumRedeemRequestMatchesForMint: Get<u32>;

		/// Unbonding slashing spans for unbonding on the relaychain.
		#[pallet::constant]
		type RelayChainUnbondingSlashingSpans: Get<u32>;

		/// Maximum number of scheduled unbonds allowed
		#[pallet::constant]
		type MaxScheduledUnbonds: Get<u32>;

		/// The number of blocks to pass before TotalStakingCurrency is updated.
		#[pallet::constant]
		type StakingUpdateFrequency: Get<Self::BlockNumber>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The total amount for the Staking currency must be more than zero.
		InvalidTotalStakingCurrency,
		/// The mint amount is below the minimum threshold allowed.
		AmountBelowMinimumThreshold,
		/// The amount of Staking currency used has exceeded the cap allowed.
		ExceededStakingCurrencyMintCap,
		/// There isn't enough reserved currencies to cancel the redeem request.
		InsufficientReservedBalances,
		/// Amount redeemed is above total amount staked.
		InsufficientTotalStakingCurrency,
		/// There isn't enough liquid balance in the user's account.
		InsufficientLiquidBalance,
		/// Too many Scheduled unbonds
		TooManyScheduledUnbonds,
		/// The xcm operation have failed
		XcmFailed,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// The user has Staked some currencies to mint Liquid Currency.
		/// \[user, amount_staked, amount_minted\]
		Minted(T::AccountId, Balance, Balance),

		/// The total amount of the staking currency on the relaychain has been
		/// set.\[total_staking_currency\]
		TotalStakingCurrencySet(Balance),

		/// The mint cap for Staking currency is updated.\[new_cap\]
		StakingCurrencyMintCapUpdated(Balance),

		/// A new weight for XCM transfers has been set.\[new_weight\]
		XcmDestWeightSet(Weight),

		/// The redeem request has been cancelled, and funds un-reserved.
		/// \[who, liquid_amount_unreserved\]
		RedeemRequestCancelled(T::AccountId, Balance),

		/// A new Redeem request has been registered.
		/// \[who, liquid_amount, extra_fee, withdraw_fee_paid\]
		RedeemRequested(T::AccountId, Balance, Permill, Balance),

		/// The user has redeemed some Liquid currency back to Staking currency.
		/// \[user, staking_amount_redeemed, liquid_amount_deducted\]
		Redeemed(T::AccountId, Balance, Balance),

		/// A new Unbond request added to the schedule.
		/// \[staking_amount, relaychain_blocknumber\]
		ScheduledUnbondAdded(Balance, RelayChainBlockNumberOf<T>),

		/// The ScheduledUnbond has been replaced.
		ScheduledUnbondReplaced,

		/// The scheduled Unbond has been withdrew from the RelayChain.
		///\[staking_amount_added\]
		ScheduledUnbondWithdrew(Balance),

		/// Interest rate for TotalStakingCurrency is set
		StakingInterestRatePerUpdateSet(Permill),

		/// The amount of the staking currency available to be redeemed is set.
		/// \[total_available_staking_balance\]
		AvailableStakingBalanceSet(Balance),
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

	/// Requests to redeem staked currencies.
	/// RedeemRequests: Map: AccountId => Option<(liquid_amount: Balance, addtional_fee: Permill)>
	#[pallet::storage]
	#[pallet::getter(fn redeem_requests)]
	pub type RedeemRequests<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, (Balance, Permill), OptionQuery>;

	/// The amount of staking currency that is available to be redeemed.
	/// AvailableStakingBalance: value: Balance
	#[pallet::storage]
	#[pallet::getter(fn available_staking_balance)]
	pub type AvailableStakingBalance<T: Config> = StorageValue<_, Balance, ValueQuery>;

	/// Funds that will be unbonded in the future
	/// ScheduledUnbond: Vec<(staking_amount: Balance, unbond_at: RelayChainBlockNumber>
	#[pallet::storage]
	#[pallet::getter(fn scheduled_unbond)]
	pub type ScheduledUnbond<T: Config> =
		StorageValue<_, BoundedVec<(Balance, RelayChainBlockNumberOf<T>), T::MaxScheduledUnbonds>, ValueQuery>;

	/// Every T::StakingUpdateFrequency blocks, TotalStakingCurrency gain interest by this rate.
	/// StakingInterestRatePerUpdate: Value: Permill
	#[pallet::storage]
	#[pallet::getter(fn staking_interest_rate_per_update)]
	pub type StakingInterestRatePerUpdate<T: Config> = StorageValue<_, Permill, ValueQuery>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_idle(_n: T::BlockNumber, remaining_weight: Weight) -> Weight {
			let mut current_weight = 0;
			// If enough weight, process the next XCM unbond.
			if remaining_weight > <T as Config>::WeightInfo::xcm_unbond() {
				let mut scheduled_unbond = Self::scheduled_unbond();
				if !scheduled_unbond.is_empty() {
					let (staking_amount, block_number) = scheduled_unbond[0];
					if T::RelayChainBlockNumber::current_block_number() >= block_number {
						let res = Self::process_scheduled_unbond(staking_amount);
						log::debug!("{:?}", res);
						debug_assert!(res.is_ok());

						if res.is_ok() {
							current_weight = <T as Config>::WeightInfo::xcm_unbond();

							scheduled_unbond.remove(0);
							ScheduledUnbond::<T>::put(scheduled_unbond);
						}
					}
				}
			}

			// With remaining weight, calculate max number of redeems that can be matched
			let num_redeem_matches = remaining_weight
				.saturating_sub(current_weight)
				.checked_div(<T as Config>::WeightInfo::redeem_with_available_staking_balance())
				.unwrap_or_default();

			// Iterate through existing redeem_requests, and try to match them with `available_staking_balance`
			let res = Self::process_redeem_requests_with_available_staking_balance(num_redeem_matches as u32);
			debug_assert!(res.is_ok());
			if let Ok(count) = res {
				current_weight = current_weight.saturating_add(
					<T as Config>::WeightInfo::redeem_with_available_staking_balance().saturating_mul(count as Weight),
				);
			}

			current_weight
		}

		fn on_initialize(n: T::BlockNumber) -> Weight {
			// Update the total amount of Staking balance by acrueing the interest periodically.
			let interest_rate = Self::staking_interest_rate_per_update();
			if !interest_rate.is_zero()
				&& n.checked_rem(&T::StakingUpdateFrequency::get())
					.map_or(false, |n| n.is_zero())
			{
				// Inflate the staking total by the interest rate.
				// This will only fail when current TotalStakingCurrency is 0. In this case it is OK to fail.
				let _ = Self::update_total_staking_currency_storage(|current| {
					Ok(current.saturating_add(interest_rate.mul(current)))
				});
				<T as Config>::WeightInfo::on_initialize()
			} else {
				<T as Config>::WeightInfo::on_initialize_without_work()
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Mint some Liquid currency, by locking up the given amount of Staking currency.
		/// Will try to match Redeem Requests if available. Remaining amount is minted via XCM.
		///
		/// The exchange rate is calculated using the ratio of the total amount of the staking and
		/// liquid currency.
		///
		/// If any amount is minted through XCM, a portion of that amount (T::MintFee and
		/// T::MaxRewardPerEra) is reducted as fee.
		///
		/// Parameters:
		/// - `amount`: The amount of Staking currency to be exchanged.
		#[pallet::weight(< T as Config >::WeightInfo::mint())]
		#[transactional]
		pub fn mint(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResult {
			let minter = ensure_signed(origin)?;

			Self::do_mint_with_requests(&minter, amount, vec![])
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
			Self::update_total_staking_currency_storage(|_n| Ok(staking_total))
		}

		/// Adjusts the total_staking_currency by the given difference.
		/// Requires `T::GovernanceOrigin`
		///
		/// Parameters:
		/// - `adjustment`: The difference in amount the total_staking_currency should be adjusted
		///   by.
		#[pallet::weight(< T as Config >::WeightInfo::adjust_total_staking_currency())]
		#[transactional]
		pub fn adjust_total_staking_currency(origin: OriginFor<T>, by_amount: AmountOf<T>) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			// Convert AmountOf<T> into Balance safely.
			let by_amount_abs = if by_amount == AmountOf::<T>::min_value() {
				AmountOf::<T>::max_value()
			} else {
				by_amount.abs()
			};

			let by_balance = TryInto::<Balance>::try_into(by_amount_abs).map_err(|_| ArithmeticError::Overflow)?;

			// ensure TotalStakingCurrency doesn't become 0
			ensure!(
				by_amount.is_positive() || by_balance < Self::total_staking_currency(),
				Error::<T>::InvalidTotalStakingCurrency
			);

			// Adjust the current total.
			Self::update_total_staking_currency_storage(|current_staking_total| {
				Ok(if by_amount.is_positive() {
					current_staking_total.saturating_add(by_balance)
				} else {
					current_staking_total.saturating_sub(by_balance)
				})
			})
		}

		/// Updates the cap for how much Staking currency can be used to Mint liquid currency.
		/// Requires `T::GovernanceOrigin`
		///
		/// Parameters:
		/// - `new_cap`: The new cap for staking currency.
		#[pallet::weight(< T as Config >::WeightInfo::set_minting_cap())]
		#[transactional]
		pub fn set_minting_cap(origin: OriginFor<T>, #[pallet::compact] new_cap: Balance) -> DispatchResult {
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
		pub fn set_xcm_dest_weight(origin: OriginFor<T>, #[pallet::compact] xcm_dest_weight: Weight) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			XcmDestWeight::<T>::put(xcm_dest_weight);
			Self::deposit_event(Event::<T>::XcmDestWeightSet(xcm_dest_weight));
			Ok(())
		}

		/// Mint some Liquid currency, by locking up the given amount of Staking currency.
		/// This is similar with the mint() extrinsic, except that the given Redeem Requests are
		/// matched with priority.
		///
		/// Parameters:
		/// - `amount`: The amount of Staking currency to be exchanged.
		/// - `requests`: The redeem requests that are prioritized to match.
		#[pallet::weight(< T as Config >::WeightInfo::mint_for_requests())]
		#[transactional]
		pub fn mint_for_requests(
			origin: OriginFor<T>,
			#[pallet::compact] amount: Balance,
			requests: Vec<T::AccountId>,
		) -> DispatchResult {
			let minter = ensure_signed(origin)?;

			Self::do_mint_with_requests(&minter, amount, requests)
		}

		/// Put in an request to redeem Staking currencies used to mint Liquid currency.
		/// The redemption will happen after the currencies are unbonded on the relaychain.
		///
		/// Parameters:
		/// - `liquid_amount`: The amount of liquid currency to be redeemed into Staking currency.
		/// - `additional_fee`: Percentage of the fee to be awarded to the minter.
		#[pallet::weight(< T as Config >::WeightInfo::request_redeem())]
		#[transactional]
		pub fn request_redeem(
			origin: OriginFor<T>,
			#[pallet::compact] liquid_amount: Balance,
			additional_fee: Permill,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			if liquid_amount.is_zero() {
				// If the amount is zero, cancel previous redeem request.
				if let Some((request_amount, _)) = RedeemRequests::<T>::take(&who) {
					// Unreserve the liquid fee and remove the redeem request.
					let unreserved = T::Currency::unreserve(T::LiquidCurrencyId::get(), &who, request_amount);
					ensure!(unreserved.is_zero(), Error::<T>::InsufficientReservedBalances);

					Self::deposit_event(Event::<T>::RedeemRequestCancelled(who, request_amount));
				}
				return Ok(());
			}

			// Redeem amount must be above a certain limit.
			ensure!(
				Self::liquid_amount_is_above_minimum_threshold(liquid_amount),
				Error::<T>::AmountBelowMinimumThreshold
			);

			// Deduct base withdraw fee and add the redeem request to the queue.
			RedeemRequests::<T>::try_mutate(&who, |request| -> DispatchResult {
				let old_amount = request.take().map(|(amount, _)| amount).unwrap_or_default();

				let diff_amount = liquid_amount.saturating_sub(old_amount);

				let base_withdraw_fee = T::BaseWithdrawFee::get().mul(diff_amount);
				if !base_withdraw_fee.is_zero() {
					// Burn withdraw fee for increased amount
					let slash_amount = T::Currency::slash(T::LiquidCurrencyId::get(), &who, base_withdraw_fee);
					ensure!(slash_amount.is_zero(), Error::<T>::InsufficientLiquidBalance);
				}

				// Deduct BaseWithdrawFee from the liquid amount.
				let liquid_amount = liquid_amount.saturating_sub(base_withdraw_fee);

				// Reserve/unreserve the difference amount.
				match liquid_amount.cmp(&old_amount) {
					// Lock more liquid currency.
					Ordering::Greater => T::Currency::reserve(
						T::LiquidCurrencyId::get(),
						&who,
						liquid_amount.saturating_sub(old_amount),
					),
					Ordering::Less => {
						// If the new amount is less, unlock the difference.
						let unserved_leftover = T::Currency::unreserve(
							T::LiquidCurrencyId::get(),
							&who,
							old_amount.saturating_sub(liquid_amount),
						);
						ensure!(unserved_leftover.is_zero(), Error::<T>::InsufficientLiquidBalance);
						Ok(())
					}
					_ => Ok(()),
				}?;

				// Set the new amount into storage.
				*request = Some((liquid_amount, additional_fee));

				Self::deposit_event(Event::<T>::RedeemRequested(
					who.clone(),
					liquid_amount,
					additional_fee,
					base_withdraw_fee,
				));

				Ok(())
			})?;

			// With redeem request added to the queue, try to redeem it with available staking balance.
			Self::process_redeem_requests_with_available_staking_balance(1)?;
			Ok(())
		}

		/// Request staking currencies to be unbonded from the RelayChain.
		///
		/// Requires `T::GovernanceOrigin`
		///
		/// Parameters:
		/// - `staking_amount`: The amount of staking currency to be unbonded.
		/// - `unbond_block`: The relaychain block number to unbond.
		#[pallet::weight(< T as Config >::WeightInfo::schedule_unbond())]
		#[transactional]
		pub fn schedule_unbond(
			origin: OriginFor<T>,
			#[pallet::compact] staking_amount: Balance,
			unbond_block: RelayChainBlockNumberOf<T>,
		) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			let mut bounded_vec = Self::scheduled_unbond();
			ensure!(
				bounded_vec.try_push((staking_amount, unbond_block)).is_ok(),
				Error::<T>::TooManyScheduledUnbonds
			);
			ScheduledUnbond::<T>::put(bounded_vec);

			Self::deposit_event(Event::<T>::ScheduledUnbondAdded(staking_amount, unbond_block));
			Ok(())
		}

		/// Replace the current storage for `ScheduledUnbond`.
		/// This should only be used to correct mistaken call of schedule_unbond or if something
		/// unexpected happened on relaychain.
		///
		/// Requires `T::GovernanceOrigin`
		///
		/// Parameters:
		/// - `new_unbonds`: The new ScheduledUnbond storage to replace the currrent storage.
		#[pallet::weight(< T as Config >::WeightInfo::replace_schedule_unbond())]
		#[transactional]
		pub fn replace_schedule_unbond(
			origin: OriginFor<T>,
			new_unbonds: Vec<(Balance, RelayChainBlockNumberOf<T>)>,
		) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			ensure!(
				new_unbonds.len() as u32 <= T::MaxScheduledUnbonds::get(),
				Error::<T>::TooManyScheduledUnbonds
			);
			let bounded_vec = BoundedVec::try_from(new_unbonds).unwrap();
			ScheduledUnbond::<T>::put(bounded_vec);

			Self::deposit_event(Event::<T>::ScheduledUnbondReplaced);

			Ok(())
		}

		/// Adjusts the AvailableStakingBalance by the given difference.
		/// Also attempt to process queued redeem request with the new Staking Balance.
		/// Requires `T::GovernanceOrigin`
		///
		/// Parameters:
		/// - `adjustment`: The difference in amount the AvailableStakingBalance should be adjusted
		///   by.
		///
		/// Weight: Weight(xcm unbond) + n * Weight(match redeem requests), where n is number of
		/// redeem requests matched.
		#[pallet::weight(
			< T as Config >::WeightInfo::adjust_available_staking_balance_with_no_matches().saturating_add(
			(*max_num_matches as Weight).saturating_mul(< T as Config >::WeightInfo::redeem_with_available_staking_balance())
			)
		)]
		#[transactional]
		pub fn adjust_available_staking_balance(
			origin: OriginFor<T>,
			by_amount: AmountOf<T>,
			max_num_matches: u32,
		) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			// Convert AmountOf<T> into Balance safely.
			let by_amount_abs = if by_amount == AmountOf::<T>::min_value() {
				AmountOf::<T>::max_value()
			} else {
				by_amount.abs()
			};

			let by_balance = TryInto::<Balance>::try_into(by_amount_abs).map_err(|_| ArithmeticError::Overflow)?;

			// Adjust the current total.
			AvailableStakingBalance::<T>::mutate(|current| {
				if by_amount.is_positive() {
					*current = current.saturating_add(by_balance);
				} else {
					*current = current.saturating_sub(by_balance);
				}
				Self::deposit_event(Event::<T>::AvailableStakingBalanceSet(*current));
			});

			// With new staking balance available, process pending redeem requests.
			Self::process_redeem_requests_with_available_staking_balance(max_num_matches)?;
			Ok(())
		}

		/// Set the interest rate for TotalStakingCurrency.
		/// TotakStakingCurrency is incremented every `T::StakingUpdateFrequency` blocks
		///
		/// Requires `T::GovernanceOrigin`
		///
		/// Parameters:
		/// - `interest_rate`: the new interest rate for TotalStakingCurrency.
		#[pallet::weight(< T as Config >::WeightInfo::set_staking_interest_rate_per_update())]
		#[transactional]
		pub fn set_staking_interest_rate_per_update(origin: OriginFor<T>, interest_rate: Permill) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			StakingInterestRatePerUpdate::<T>::put(interest_rate);

			Self::deposit_event(Event::<T>::StakingInterestRatePerUpdateSet(interest_rate));

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Calculate the amount of Staking currency converted from Liquid currency.
		/// staking_amount = (total_staking_amount / liquid_total_issuance) * liquid_amount
		/// If the exchange rate cannot be calculated, T::DefaultExchangeRate is used
		pub fn convert_liquid_to_staking(liquid_amount: Balance) -> Result<Balance, DispatchError> {
			Self::get_exchange_rate()
				.checked_mul_int(liquid_amount)
				.ok_or(DispatchError::Arithmetic(ArithmeticError::Overflow))
		}

		/// Calculate the amount of Liquid currency converted from Staking currency.
		/// liquid_amount = (liquid_total_issuance / total_staking_amount) * staking_amount
		/// If the exchange rate cannot be calculated, T::DefaultExchangeRate is used
		pub fn convert_staking_to_liquid(staking_amount: Balance) -> Result<Balance, DispatchError> {
			Self::get_exchange_rate()
				.reciprocal()
				.unwrap_or_else(|| T::DefaultExchangeRate::get().reciprocal().unwrap())
				.checked_mul_int(staking_amount)
				.ok_or(DispatchError::Arithmetic(ArithmeticError::Overflow))
		}

		/// Match a redeem request with a mint request. Attempt to redeem as much as possible.
		///
		/// If the redeemer doesn't have enough liquid currency, do nothing. Otherwise:
		///
		/// Transfer a reduced amount of Staking currency from the Minter to the Redeemer.
		/// Transfer the full amount of Liquid currency from Redeemer to Minter.
		/// Modify `liquid_amount_remaining` and store new RedeemRequest balances in `new_balances`.
		/// Deposit the "Redeemed" event.
		///
		/// NOTE: the `RedeemRequest` storage is NOT updated. New balance is pushed into
		/// `new_balances`, and should be processed after this.
		///
		/// Param:
		/// - `minter`: The AccountId requested the Mint
		/// - `redeemer`: The AccountId requested the Redeem
		/// - `request_amount`: The RedeemRequest's amount
		/// - `request_extra_fee`: The RedeemRequest's extra fee
		/// - `liquid_amount_remaining`: The amount of liquid currency still remain to be minted.
		///   Only redeem up to this amount.
		/// - `new_balances`: Stores the new `RedeemRequest` balances. This should be iterated after
		///   to update the actual storage in bulk. Actual `RedeemRequest` storage is NOT modified
		///   here.
		fn match_mint_with_redeem_request(
			minter: &T::AccountId,
			redeemer: &T::AccountId,
			request_amount: Balance,
			request_extra_fee: Permill,
			liquid_amount_remaining: &mut Balance,
			new_balances: &mut Vec<(T::AccountId, Balance, Permill)>,
		) -> DispatchResult {
			let liquid_amount_can_be_redeemed = min(request_amount, *liquid_amount_remaining);

			// Ensure the redeemer have enough liquid currency in their account.
			if T::Currency::reserved_balance(T::LiquidCurrencyId::get(), redeemer) >= liquid_amount_can_be_redeemed {
				let new_amount = request_amount.saturating_sub(liquid_amount_can_be_redeemed);
				*liquid_amount_remaining = liquid_amount_remaining.saturating_sub(liquid_amount_can_be_redeemed);

				// Full amount of Liquid is transferred to the minter.
				// The redeemer is guaranteed to have enough reserved balance for the repatriate.
				T::Currency::repatriate_reserved(
					T::LiquidCurrencyId::get(),
					redeemer,
					minter,
					liquid_amount_can_be_redeemed,
					BalanceStatus::Free,
				)?;

				// The extra_fee is rewarded to the minter. Minter gets to keep it instead of transferring it to the
				// redeemer. staking_amount = original_staking_amount * ( 1 - additional_fee )
				let mut staking_amount = Self::convert_liquid_to_staking(liquid_amount_can_be_redeemed)?;
				let fee_deducted_percentage = Permill::one().saturating_sub(request_extra_fee);
				staking_amount = fee_deducted_percentage.mul(staking_amount);

				// Transfer the reduced staking currency from Minter to Redeemer
				T::Currency::transfer(T::StakingCurrencyId::get(), minter, redeemer, staking_amount)?;

				new_balances.push((redeemer.clone(), new_amount, request_extra_fee));
				Self::deposit_event(Event::<T>::Redeemed(
					redeemer.clone(),
					staking_amount,
					liquid_amount_can_be_redeemed,
				));
			}

			Ok(())
		}

		/// Mint some Liquid currency, by locking up the given amount of Staking currency.
		/// The redeem requests given in `requests` are prioritized to be matched. All other redeem
		/// requests are matched after. The remaining amount is minted through Staking on the
		/// RelayChain (via XCM).
		///
		/// Parameters:
		/// - `amount`: The amount of Staking currency to be exchanged.
		/// - `requests`: The redeem requests that are prioritized to match.
		fn do_mint_with_requests(
			minter: &T::AccountId,
			amount: Balance,
			requests: Vec<T::AccountId>,
		) -> DispatchResult {
			// Ensure the amount is above the minimum, after the MintFee is deducted.
			ensure!(
				amount > T::MinimumMintThreshold::get().saturating_add(T::MintFee::get()),
				Error::<T>::AmountBelowMinimumThreshold
			);

			let staking_currency = T::StakingCurrencyId::get();

			// ensure the user has enough funds on their account.
			T::Currency::ensure_can_withdraw(staking_currency, minter, amount)?;

			// Attempt to match redeem requests if there are any.
			let total_liquid_to_mint = Self::convert_staking_to_liquid(amount)?;

			// The amount of liquid currency to be redeemed for the mint reuqest.
			let mut liquid_remaining = total_liquid_to_mint;

			// New balances after redeem requests are fullfilled.
			let mut new_balances: Vec<(T::AccountId, Balance, Permill)> = vec![];

			// Iterate through the prioritized requests first
			for redeemer in requests {
				// If all the currencies are minted, return.
				if liquid_remaining.is_zero() {
					break;
				}

				// Check if the redeem request exists
				if let Some((request_amount, extra_fee)) = Self::redeem_requests(&redeemer) {
					Self::match_mint_with_redeem_request(
						minter,
						&redeemer,
						request_amount,
						extra_fee,
						&mut liquid_remaining,
						&mut new_balances,
					)?;
				}
			}

			// Update storage to the new balances. Remove Redeem requests that have been filled.
			Self::update_redeem_requests(&new_balances);

			// Redeem request storage has now been updated.
			new_balances.clear();

			let mut redeem_requests_limit_remaining = T::MaximumRedeemRequestMatchesForMint::get();
			// Iterate all remaining redeem requests now.
			for (redeemer, (request_amount, extra_fee)) in RedeemRequests::<T>::iter() {
				// If all the currencies are minted, return.
				if liquid_remaining.is_zero() || redeem_requests_limit_remaining.is_zero() {
					break;
				}
				Self::match_mint_with_redeem_request(
					minter,
					&redeemer,
					request_amount,
					extra_fee,
					&mut liquid_remaining,
					&mut new_balances,
				)?;
				redeem_requests_limit_remaining -= 1;
			}

			// Update storage to the new balances. Remove Redeem requests that have been filled.
			Self::update_redeem_requests(&new_balances);

			// If significant balance is left over, the remaining liquid currencies are minted through XCM.
			let mut staking_remaining = Self::convert_liquid_to_staking(liquid_remaining)?;
			if staking_remaining > T::MinimumMintThreshold::get().saturating_add(T::MintFee::get()) {
				// Calculate how much Liquid currency is to be minted.
				// liquid_to_mint = convert_to_liquid( (staked_amount - MintFee) * (1 - MaxRewardPerEra) )
				let mut liquid_to_mint = staking_remaining
					.checked_sub(T::MintFee::get())
					.expect("Mint amount is ensured to be greater than T::MintFee; qed");
				liquid_to_mint = (Permill::one().saturating_sub(T::MaxRewardPerEra::get())).mul(liquid_to_mint);
				liquid_to_mint = Self::convert_staking_to_liquid(liquid_to_mint)?;

				// Update staking total and ensure the new total doesn't exceed the cap.
				Self::update_total_staking_currency_storage(|total_staking_currency| {
					let new_total_staking_currency = total_staking_currency
						.checked_add(staking_remaining)
						.ok_or(ArithmeticError::Overflow)?;
					ensure!(
						new_total_staking_currency <= Self::staking_currency_mint_cap(),
						Error::<T>::ExceededStakingCurrencyMintCap
					);
					Ok(new_total_staking_currency)
				})?;

				// All checks pass. Proceed with Xcm transfer.
				T::XcmTransfer::transfer(
					minter.clone(),
					staking_currency,
					staking_remaining,
					T::SovereignSubAccountLocation::get(),
					Self::xcm_dest_weight(),
				)?;
				T::Currency::deposit(T::LiquidCurrencyId::get(), minter, liquid_to_mint)?;

				staking_remaining = Balance::zero();
				liquid_remaining = liquid_remaining.saturating_sub(liquid_to_mint);
			}

			let actual_staked = amount.saturating_sub(staking_remaining);
			let actual_liquid = total_liquid_to_mint.saturating_sub(liquid_remaining);

			Self::deposit_event(Event::<T>::Minted(minter.clone(), actual_staked, actual_liquid));

			Ok(())
		}

		/// Construct XCM message and sent it to the relaychain to withdraw_unbonded Staking
		/// currency. The staking currency withdrew becomes available to be redeemed.
		///
		/// params:
		/// 	- `staking_amount_unbonded`: amount of staking currency to withdraw unbond via XCM
		#[transactional]
		pub fn process_scheduled_unbond(staking_amount_unbonded: Balance) -> DispatchResult {
			let msg = Self::construct_xcm_unreserve_message(T::ParachainAccount::get(), staking_amount_unbonded);

			let res = pallet_xcm::Pallet::<T>::send_xcm(Here, Parent, msg);
			log::debug!("on_idle XCM result: {:?}", res);
			ensure!(res.is_ok(), Error::<T>::XcmFailed);

			// Update storage with the new available amount
			AvailableStakingBalance::<T>::mutate(|current| {
				*current = current.saturating_add(staking_amount_unbonded);
			});

			Self::deposit_event(Event::<T>::ScheduledUnbondWithdrew(staking_amount_unbonded));
			Ok(())
		}

		/// Iterate through all redeem requests, then match them with available_staking_balance.
		/// This should be called when new available_staking_balance becomes available.
		///
		/// params:
		/// 	- `max_num_matches`: Maximum number of redeem requests to be matched.
		///
		/// return:
		/// 	Result<u32, DispatchError>: The number of redeem reqeusts actually matched.
		#[transactional]
		pub fn process_redeem_requests_with_available_staking_balance(
			max_num_matches: u32,
		) -> Result<u32, sp_runtime::DispatchError> {
			if max_num_matches.is_zero() {
				return Ok(0);
			}
			let mut available_staking_balance = Self::available_staking_balance();
			if available_staking_balance <= T::MinimumMintThreshold::get() {
				return Ok(0);
			}

			let mut new_balances: Vec<(T::AccountId, Balance, Permill)> = vec![];
			let mut num_matched = 0u32;
			for (redeemer, (request_amount, extra_fee)) in RedeemRequests::<T>::iter() {
				let actual_liquid_amount = min(
					request_amount,
					Self::convert_staking_to_liquid(available_staking_balance)?,
				);

				// Ensure the redeemer have enough liquid currency in their account.
				if T::Currency::reserved_balance(T::LiquidCurrencyId::get(), &redeemer) >= actual_liquid_amount {
					let actual_staking_amount = Self::convert_liquid_to_staking(actual_liquid_amount)?;

					Self::update_total_staking_currency_storage(|total| {
						Ok(total.saturating_sub(actual_staking_amount))
					})?;

					//Actual deposit amount has `T::XcmUnbondFee` deducted.
					let actual_staking_amount_deposited = actual_staking_amount.saturating_sub(T::XcmUnbondFee::get());
					T::Currency::deposit(T::StakingCurrencyId::get(), &redeemer, actual_staking_amount_deposited)?;

					// Burn the corresponding amount of Liquid currency from the user.
					// The redeemer is guaranteed to have enough fund
					T::Currency::unreserve(T::LiquidCurrencyId::get(), &redeemer, actual_liquid_amount);
					T::Currency::slash(T::LiquidCurrencyId::get(), &redeemer, actual_liquid_amount);

					available_staking_balance = available_staking_balance.saturating_sub(actual_staking_amount);
					let request_amount_remaining = request_amount.saturating_sub(actual_liquid_amount);
					new_balances.push((redeemer.clone(), request_amount_remaining, extra_fee));

					Self::deposit_event(Event::<T>::Redeemed(
						redeemer,
						actual_staking_amount_deposited,
						actual_liquid_amount,
					));
					num_matched += 1u32;
				}

				// If all the currencies are minted, return.
				if available_staking_balance < T::MinimumMintThreshold::get() || num_matched >= max_num_matches {
					break;
				}
			}

			// Update storage to the new balances. Remove Redeem requests that have been filled.
			Self::update_redeem_requests(&new_balances);

			AvailableStakingBalance::<T>::put(available_staking_balance);

			Ok(num_matched)
		}

		/// Update the RedeemRequests storage with the new balances.
		/// Remove Redeem requests that are dust, or have been filled.
		#[allow(clippy::ptr_arg)]
		fn update_redeem_requests(new_balances: &Vec<(T::AccountId, Balance, Permill)>) {
			for (redeemer, new_balance, extra_fee) in new_balances {
				if Self::liquid_amount_is_above_minimum_threshold(*new_balance) {
					RedeemRequests::<T>::insert(&redeemer, (*new_balance, *extra_fee));
				} else {
					if !new_balance.is_zero() {
						// Unlock the dust and remove the request.
						T::Currency::unreserve(T::LiquidCurrencyId::get(), redeemer, *new_balance);
					}
					RedeemRequests::<T>::remove(&redeemer);
				}
			}
		}

		fn liquid_amount_is_above_minimum_threshold(liquid_amount: Balance) -> bool {
			liquid_amount > T::MinimumRedeemThreshold::get()
				&& Self::convert_liquid_to_staking(liquid_amount).unwrap_or_default() > T::XcmUnbondFee::get()
		}

		/// Construct a XCM message
		pub fn construct_xcm_unreserve_message(parachain_account: T::AccountId, amount: Balance) -> Xcm<()> {
			let xcm_message = T::RelayChainCallBuilder::utility_as_derivative_call(
				T::RelayChainCallBuilder::utility_batch_call(vec![
					T::RelayChainCallBuilder::staking_withdraw_unbonded(T::RelayChainUnbondingSlashingSpans::get()),
					T::RelayChainCallBuilder::balances_transfer_keep_alive(parachain_account, amount),
				]),
				T::SubAccountIndex::get(),
			);
			T::RelayChainCallBuilder::finalize_call_into_xcm_message(
				xcm_message,
				T::XcmUnbondFee::get(),
				Self::xcm_dest_weight(),
			)
		}

		/// Helper function that update the storage of total_staking_currency and emit event.
		fn update_total_staking_currency_storage(
			f: impl FnOnce(Balance) -> Result<Balance, DispatchError>,
		) -> DispatchResult {
			TotalStakingCurrency::<T>::try_mutate(|current| {
				*current = f(*current)?;
				ensure!(!current.is_zero(), Error::<T>::InvalidTotalStakingCurrency);
				Self::deposit_event(Event::<T>::TotalStakingCurrencySet(*current));
				Ok(())
			})
		}
	}

	impl<T: Config> ExchangeRateProvider for Pallet<T> {
		/// Calculate the exchange rate between the Staking and Liquid currency.
		/// returns Ratio(staking : liquid) = total_staking_amount / liquid_total_issuance
		/// If the exchange rate cannot be calculated, T::DefaultExchangeRate is used
		fn get_exchange_rate() -> Ratio {
			let staking_total = Self::total_staking_currency();
			let liquid_total = T::Currency::total_issuance(T::LiquidCurrencyId::get());
			if staking_total.is_zero() {
				T::DefaultExchangeRate::get()
			} else {
				Ratio::checked_from_rational(staking_total, liquid_total).unwrap_or_else(T::DefaultExchangeRate::get)
			}
		}
	}
}
