// This file is part of Acala.

// Copyright (C) 2020-2022 Acala Foundation.
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

	#[derive(RuntimeDebug, Clone, Copy, PartialEq)]
	pub enum RedeemType<AccountId> {
		WithAvailableStakingBalance,
		WithMint(AccountId, Balance),
	}

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
		type HomaUnbondFee: Get<Balance>;

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
		Minted {
			who: T::AccountId,
			amount_staked: Balance,
			amount_minted: Balance,
		},

		/// The total amount of the staking currency on the relaychain has been set.
		TotalStakingCurrencySet { total_staking_currency: Balance },

		/// The mint cap for Staking currency is updated.
		StakingCurrencyMintCapUpdated { new_cap: Balance },

		/// A new weight for XCM transfers has been set.
		XcmDestWeightSet { new_weight: Weight },

		/// The redeem request has been cancelled, and funds un-reserved.
		RedeemRequestCancelled {
			who: T::AccountId,
			liquid_amount_unreserved: Balance,
		},

		/// A new Redeem request has been registered.
		RedeemRequested {
			who: T::AccountId,
			liquid_amount: Balance,
			extra_fee: Permill,
			withdraw_fee_paid: Balance,
		},

		/// The user has redeemed some Liquid currency back to Staking currency.
		Redeemed {
			who: T::AccountId,
			staking_amount_redeemed: Balance,
			liquid_amount_deducted: Balance,
		},

		/// A new Unbond request added to the schedule.
		ScheduledUnbondAdded {
			staking_amount: Balance,
			relaychain_blocknumber: RelayChainBlockNumberOf<T>,
		},

		/// The ScheduledUnbond has been replaced.
		ScheduledUnbondReplaced,

		/// The scheduled Unbond has been withdrew from the RelayChain.
		ScheduledUnbondWithdrew { staking_amount_added: Balance },

		/// Interest rate for TotalStakingCurrency is set
		StakingInterestRatePerUpdateSet { interest_rate: Permill },

		/// The amount of the staking currency available to be redeemed is set.
		AvailableStakingBalanceSet { total_available_staking_balance: Balance },
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
	/// RedeemRequests: Map: AccountId => Option<(liquid_amount: Balance, additional_fee: Permill)>
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

	/// Next redeem request to iterate from when matching redeem requests.
	/// LastRedeemRequestKeyIterated: Value: Vec<u8>
	#[pallet::storage]
	#[pallet::getter(fn last_redeem_request_key_iterated)]
	pub type LastRedeemRequestKeyIterated<T: Config> = StorageValue<_, Vec<u8>, ValueQuery>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
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
			let res = Self::redeem_from_previous_redeem_request(
				RedeemType::WithAvailableStakingBalance,
				num_redeem_matches as u32,
			);
			debug_assert!(res.is_ok());
			if let Ok((_, count)) = res {
				current_weight = current_weight.saturating_add(
					<T as Config>::WeightInfo::redeem_with_available_staking_balance().saturating_mul(count as Weight),
				);
			}

			current_weight
		}

		fn on_initialize(n: T::BlockNumber) -> Weight {
			// Update the total amount of Staking balance by accruing the interest periodically.
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

		// ensure that minimum_mint_redeem_amount * (1 - withdraw fee) > xcm unbond fee
		fn integrity_test() {
			sp_std::if_std! {
				sp_io::TestExternalities::new_empty().execute_with(||
					assert!(
						Permill::one().saturating_sub(T::BaseWithdrawFee::get()).mul(
						T::MinimumRedeemThreshold::get()) > T::HomaUnbondFee::get()
					));
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
		/// T::MaxRewardPerEra) is deducted as fee.
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
			Self::deposit_event(Event::<T>::StakingCurrencyMintCapUpdated { new_cap });
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
			Self::deposit_event(Event::<T>::XcmDestWeightSet {
				new_weight: xcm_dest_weight,
			});
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
					let remaining = T::Currency::unreserve(T::LiquidCurrencyId::get(), &who, request_amount);
					ensure!(remaining.is_zero(), Error::<T>::InsufficientReservedBalances);

					Self::deposit_event(Event::<T>::RedeemRequestCancelled {
						who,
						liquid_amount_unreserved: request_amount,
					});
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
						let remaining = T::Currency::unreserve(
							T::LiquidCurrencyId::get(),
							&who,
							old_amount.saturating_sub(liquid_amount),
						);
						ensure!(remaining.is_zero(), Error::<T>::InsufficientLiquidBalance);
						Ok(())
					}
					_ => Ok(()),
				}?;

				// Set the new amount into storage.
				*request = Some((liquid_amount, additional_fee));

				Self::deposit_event(Event::<T>::RedeemRequested {
					who: who.clone(),
					liquid_amount,
					extra_fee: additional_fee,
					withdraw_fee_paid: base_withdraw_fee,
				});

				Ok(())
			})?;

			// With redeem request added to the queue, try to redeem it with available staking balance.
			Self::process_redeem_requests_with_available_staking_balance(&who)?;
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

			Self::deposit_event(Event::<T>::ScheduledUnbondAdded {
				staking_amount,
				relaychain_blocknumber: unbond_block,
			});
			Ok(())
		}

		/// Replace the current storage for `ScheduledUnbond`.
		/// This should only be used to correct mistaken call of schedule_unbond or if something
		/// unexpected happened on relaychain.
		///
		/// Requires `T::GovernanceOrigin`
		///
		/// Parameters:
		/// - `new_unbonds`: The new ScheduledUnbond storage to replace the current storage.
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
				Self::deposit_event(Event::<T>::AvailableStakingBalanceSet {
					total_available_staking_balance: *current,
				});
			});

			// With new staking balance available, process pending redeem requests.
			Self::redeem_from_previous_redeem_request(RedeemType::WithAvailableStakingBalance, max_num_matches)?;
			Ok(())
		}

		/// Set the interest rate for TotalStakingCurrency.
		/// TotalStakingCurrency is incremented every `T::StakingUpdateFrequency` blocks
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

			Self::deposit_event(Event::<T>::StakingInterestRatePerUpdateSet { interest_rate });

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
		/// Modify `RedeemRequests` with updated redeem amount and deposit "Redeemed" event.
		///
		/// Param:
		/// - `minter`: The AccountId requested the Mint
		/// - `redeemer`: The AccountId requested the Redeem
		/// - `liquid_amount_to_redeem`: The amount of liquid currency still remain to be minted.
		///   Only redeem up to this amount.
		///
		/// Return:
		/// - `Balance`: Actual amount of liquid currency minted.
		fn match_mint_with_redeem_request(
			minter: &T::AccountId,
			redeemer: &T::AccountId,
			liquid_amount_to_redeem: Balance,
		) -> Result<Balance, DispatchError> {
			RedeemRequests::<T>::mutate_exists(&redeemer, |request| {
				let (request_amount, extra_fee) = request.unwrap_or_default();
				// If the redeem request doesn't exist, return.
				if request_amount.is_zero() {
					return Ok(0);
				}

				let actual_liquid_amount = min(request_amount, liquid_amount_to_redeem);

				// Ensure the redeemer have enough liquid currency in their account.
				if T::Currency::reserved_balance(T::LiquidCurrencyId::get(), redeemer) < actual_liquid_amount {
					return Ok(0);
				}

				// The extra_fee is rewarded to the minter. Minter gets to keep it instead of transferring it to the
				// redeemer. staking_amount = original_staking_amount * ( 1 - additional_fee )
				let mut staking_amount = Self::convert_liquid_to_staking(actual_liquid_amount)?;
				let fee_deducted_percentage = Permill::one().saturating_sub(extra_fee);
				staking_amount = fee_deducted_percentage.mul(staking_amount);

				// Full amount of Liquid is transferred to the minter.
				// The redeemer is guaranteed to have enough reserved balance for the repatriate.
				T::Currency::repatriate_reserved(
					T::LiquidCurrencyId::get(),
					redeemer,
					minter,
					actual_liquid_amount,
					BalanceStatus::Free,
				)?;

				// Transfer the reduced staking currency from Minter to Redeemer
				T::Currency::transfer(T::StakingCurrencyId::get(), minter, redeemer, staking_amount)?;

				Self::deposit_event(Event::<T>::Redeemed {
					who: redeemer.clone(),
					staking_amount_redeemed: staking_amount,
					liquid_amount_deducted: actual_liquid_amount,
				});

				// Update storage
				let new_amount = request_amount.saturating_sub(actual_liquid_amount);
				if Self::liquid_amount_is_above_minimum_threshold(new_amount) {
					*request = Some((new_amount, extra_fee));
				} else {
					// Unlock the dust and remove the request.
					if !new_amount.is_zero() {
						T::Currency::unreserve(T::LiquidCurrencyId::get(), redeemer, new_amount);
					}
					*request = None;
				}
				Ok(actual_liquid_amount)
			})
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

			// The amount of liquid currency to be redeemed for the mint request.
			let mut liquid_remaining = total_liquid_to_mint;

			// Iterate through the prioritized requests first
			for redeemer in requests {
				// If all the currencies are minted, return.
				if liquid_remaining.is_zero() {
					break;
				}

				// Check if the redeem request exists
				if Self::redeem_requests(&redeemer).is_some() {
					let actual_liquid_redeemed =
						Self::match_mint_with_redeem_request(minter, &redeemer, liquid_remaining)?;
					liquid_remaining = liquid_remaining.saturating_sub(actual_liquid_redeemed);
				}
			}

			// Iterate through the rest of the RedeemRequests to mint
			let redeem_requests_limit_remaining = T::MaximumRedeemRequestMatchesForMint::get();
			if !liquid_remaining.is_zero() && !redeem_requests_limit_remaining.is_zero() {
				let (liquid_amount_redeemed, _) = Self::redeem_from_previous_redeem_request(
					RedeemType::WithMint(minter.clone(), liquid_remaining),
					redeem_requests_limit_remaining,
				)?;
				liquid_remaining = liquid_remaining.saturating_sub(liquid_amount_redeemed);
			}

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

			Self::deposit_event(Event::<T>::Minted {
				who: minter.clone(),
				amount_staked: actual_staked,
				amount_minted: actual_liquid,
			});

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

			Self::deposit_event(Event::<T>::ScheduledUnbondWithdrew {
				staking_amount_added: staking_amount_unbonded,
			});
			Ok(())
		}

		/// Redeem the given requests with available_staking_balance.
		///
		/// params:
		///  - `redeemer`: The account Id of the redeem requester
		///
		/// return:
		///  -`Result<actual_amount_redeemed, DispatchError>`: The liquid amount actually redeemed.
		#[transactional]
		pub fn process_redeem_requests_with_available_staking_balance(
			redeemer: &T::AccountId,
		) -> Result<Balance, DispatchError> {
			let available_staking_balance = Self::available_staking_balance();
			if available_staking_balance <= T::MinimumMintThreshold::get() {
				return Ok(0);
			}

			RedeemRequests::<T>::mutate_exists(&redeemer, |request| {
				let (request_amount, extra_fee) = request.unwrap_or_default();
				// If the redeem request doesn't exist, return.
				if request_amount.is_zero() {
					// this should not happen, but if it does, do some cleanup
					*request = None;
					return Ok(0);
				}

				let actual_liquid_amount = min(
					request_amount,
					Self::convert_staking_to_liquid(available_staking_balance)?,
				);

				// Ensure the redeemer have enough liquid currency in their account.
				if T::Currency::reserved_balance(T::LiquidCurrencyId::get(), redeemer) < actual_liquid_amount {
					return Ok(0);
				}
				let actual_staking_amount = Self::convert_liquid_to_staking(actual_liquid_amount)?;

				Self::update_total_staking_currency_storage(|total| Ok(total.saturating_sub(actual_staking_amount)))?;

				//Actual deposit amount has `T::HomaUnbondFee` deducted.
				let actual_staking_amount_deposited = actual_staking_amount.saturating_sub(T::HomaUnbondFee::get());
				T::Currency::deposit(T::StakingCurrencyId::get(), redeemer, actual_staking_amount_deposited)?;

				// Burn the corresponding amount of Liquid currency from the user.
				// The redeemer is guaranteed to have enough fund
				let unslashed = T::Currency::slash_reserved(T::LiquidCurrencyId::get(), redeemer, actual_liquid_amount);
				debug_assert!(unslashed.is_zero());

				AvailableStakingBalance::<T>::mutate(|current| {
					*current = current.saturating_sub(actual_staking_amount)
				});

				Self::deposit_event(Event::<T>::Redeemed {
					who: redeemer.clone(),
					staking_amount_redeemed: actual_staking_amount_deposited,
					liquid_amount_deducted: actual_liquid_amount,
				});

				// Update storage
				let new_amount = request_amount.saturating_sub(actual_liquid_amount);
				if Self::liquid_amount_is_above_minimum_threshold(new_amount) {
					*request = Some((new_amount, extra_fee));
				} else {
					// Unlock the dust and remove the request.
					if !new_amount.is_zero() {
						let remaining = T::Currency::unreserve(T::LiquidCurrencyId::get(), redeemer, new_amount);
						debug_assert!(remaining.is_zero());
					}
					*request = None;
				}

				Ok(actual_liquid_amount)
			})
		}

		// Helper function that checks if the `liquid_amount` is above the minimum redeem threshold, and
		// is enough to pay for the XCM unbond fee.
		fn liquid_amount_is_above_minimum_threshold(liquid_amount: Balance) -> bool {
			liquid_amount > T::MinimumRedeemThreshold::get()
		}

		/// Helper function that construct an XCM message that:
		/// 1. `withdraw_unbonded` from HomaLite sub-account.
		/// 2. Transfer the withdrew fund into Sovereign account.
		///
		/// Param:
		/// 	- `parachain_account` : sovereign account's AccountId
		/// 	- `amount` : amount to withdraw from unbonded.
		/// Return:
		/// 	Xcm<()>: the Xcm message constructed.
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
				T::HomaUnbondFee::get(),
				Self::xcm_dest_weight(),
			)
		}

		/// Helper function that update the storage of total_staking_currency.
		/// Ensures that the total staking amount would not become zero, and emit an event.
		fn update_total_staking_currency_storage(
			f: impl FnOnce(Balance) -> Result<Balance, DispatchError>,
		) -> DispatchResult {
			TotalStakingCurrency::<T>::try_mutate(|current| {
				*current = f(*current)?;
				ensure!(!current.is_zero(), Error::<T>::InvalidTotalStakingCurrency);
				Self::deposit_event(Event::<T>::TotalStakingCurrencySet {
					total_staking_currency: *current,
				});
				Ok(())
			})
		}

		/// Function that iterates `RedeemRequests` storage from `LastRedeemRequestKeyIterated`, and
		/// redeem them 	depending on the redeem type. Either redeem from AvailableStakingBalance, or
		/// from a specific minter.
		/// If the item after `LastRedeemRequestKeyIterated` is the end of the iterator, then start
		/// from the beginning.
		///
		/// Param:
		/// - `redeem_type`: How redeem should happen.
		/// - `max_num_matches`: Maximum number of requests to be redeemed.
		///
		/// Return:
		/// - `total_amount_redeemed`: the total amount of liquid actually redeemed
		/// - `num_matched`: number of requests actually redeemed.
		pub fn redeem_from_previous_redeem_request(
			redeem_type: RedeemType<T::AccountId>,
			max_num_matches: u32,
		) -> Result<(Balance, u32), DispatchError> {
			let starting_key = Self::last_redeem_request_key_iterated();
			let mut iterator = RedeemRequests::<T>::iter_keys_from(starting_key);

			let mut redeem_amount_remaining = if let RedeemType::WithMint(_, amount) = redeem_type {
				amount
			} else {
				0
			};

			let mut total_amount_redeemed: Balance = 0;
			let mut num_matched = 0u32;
			let mut finished_iteration = false;

			let mut body = |redeemer: T::AccountId| -> sp_std::result::Result<bool, DispatchError> {
				if num_matched >= max_num_matches {
					return Ok(true);
				}
				num_matched += 1;

				match &redeem_type {
					RedeemType::WithAvailableStakingBalance => {
						let amount_redeemed = Self::process_redeem_requests_with_available_staking_balance(&redeemer)?;
						total_amount_redeemed = total_amount_redeemed.saturating_add(amount_redeemed);
						if Self::available_staking_balance() <= T::MinimumMintThreshold::get() {
							return Ok(true);
						}
					}
					RedeemType::WithMint(minter, _) => {
						let amount_redeemed =
							Self::match_mint_with_redeem_request(minter, &redeemer, redeem_amount_remaining)?;
						total_amount_redeemed = total_amount_redeemed.saturating_add(amount_redeemed);
						redeem_amount_remaining = redeem_amount_remaining.saturating_sub(amount_redeemed);
						if !Self::liquid_amount_is_above_minimum_threshold(redeem_amount_remaining) {
							return Ok(true);
						}
					}
				}
				Ok(false)
			};

			#[allow(clippy::while_let_on_iterator)]
			while let Some(redeemer) = iterator.next() {
				if body(redeemer)? {
					finished_iteration = true;
					break;
				}
			}

			if !finished_iteration {
				iterator = RedeemRequests::<T>::iter_keys();

				#[allow(clippy::while_let_on_iterator)]
				while let Some(redeemer) = iterator.next() {
					if body(redeemer)? {
						break;
					}
				}
			}

			// Store the progress of the iterator
			LastRedeemRequestKeyIterated::<T>::put(iterator.last_raw_key());
			Ok((total_amount_redeemed, num_matched))
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
