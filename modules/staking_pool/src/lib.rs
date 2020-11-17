#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, ensure,
	traits::{EnsureOrigin, Get},
	transactional,
	weights::DispatchClass,
	IterableStorageDoubleMap,
};
use frame_system::{self as system};
use orml_traits::{Change, MultiCurrency};
use primitives::{Balance, CurrencyId, EraIndex};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{
	traits::{AccountIdConversion, CheckedDiv, One, Saturating, Zero},
	DispatchError, DispatchResult, FixedPointNumber, ModuleId, RuntimeDebug,
};
use sp_std::prelude::*;
use support::{
	ExchangeRate, HomaProtocol, NomineesProvider, OnNewEra, PolkadotBridge, PolkadotBridgeCall, PolkadotBridgeState,
	PolkadotBridgeType, PolkadotStakingLedger, PolkadotUnlockChunk, Rate, Ratio,
};

mod mock;
mod tests;

/// The params related to rebalance per era
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Params {
	pub target_max_free_unbonded_ratio: Ratio,
	pub target_min_free_unbonded_ratio: Ratio,
	pub target_unbonding_to_free_ratio: Ratio,
	pub unbonding_to_free_adjustment: Rate,
	pub base_fee_rate: Rate,
}

pub trait FeeModel<Balance> {
	fn get_fee(
		remain_available_percent: Ratio,
		available_amount: Balance,
		request_amount: Balance,
		base_rate: Rate,
	) -> Option<Balance>;
}

type ChangeRate = Change<Rate>;
type ChangeRatio = Change<Ratio>;

type PolkadotAccountIdOf<T> =
	<<T as Trait>::Bridge as PolkadotBridgeType<<T as system::Trait>::BlockNumber, EraIndex>>::PolkadotAccountId;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

	/// The staking currency id(should be DOT in acala)
	type StakingCurrencyId: Get<CurrencyId>;

	/// The liquid currency id(should be LDOT in acala)
	type LiquidCurrencyId: Get<CurrencyId>;

	/// The default exchange rate for liquid currency to staking currency.
	type DefaultExchangeRate: Get<ExchangeRate>;

	/// The staking pool's module id, keep all staking currency belong to Homa
	/// protocol.
	type ModuleId: Get<ModuleId>;

	/// The sub account indexs of parachain to vault assets of Homa protocol in
	/// Polkadot.
	type PoolAccountIndexes: Get<Vec<u32>>;

	/// The origin which may update parameters. Root can always do this.
	type UpdateOrigin: EnsureOrigin<Self::Origin>;

	/// Calculation model for unbond fees
	type FeeModel: FeeModel<Balance>;

	/// The nominees selected by governance of Homa protocol.
	type Nominees: NomineesProvider<PolkadotAccountIdOf<Self>>;

	/// The Bridge to do accross-chain operations between parachain and
	/// relaychain.
	type Bridge: PolkadotBridge<Self::AccountId, Self::BlockNumber, Balance, EraIndex>;

	/// The currency for managing assets related to Homa protocol.
	type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;
}

decl_event!(
	pub enum Event<T>
	where
		<T as system::Trait>::AccountId,
		Balance = Balance,
	{
		/// \[who, bond_staking, issued_liquid\]
		MintLiquid(AccountId, Balance, Balance),
		/// \[who, redeem_amount, unbond_amount\]
		RedeemByUnbond(AccountId, Balance, Balance),
		/// \[who, fee_in_liquid, liquid_amount_burned, staking_amount_retrived\]
		RedeemByFreeUnbonded(AccountId, Balance, Balance, Balance),
		/// \[who, target_era, fee, redeem_amount, unbond_amount\]
		RedeemByClaimUnbonding(AccountId, EraIndex, Balance, Balance, Balance),
	}
);

decl_error! {
	/// Error for staking pool module.
	pub enum Error for Module<T: Trait> {
		LiquidCurrencyNotEnough,
		InvalidEra,
		Overflow,
		GetFeeFailed,
		InvalidConfig,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as StakingPool {
		pub CurrentEra get(fn current_era): EraIndex;

		pub NextEraUnbond get(fn next_era_unbond): (Balance, Balance);
		pub Unbonding get(fn unbonding): map hasher(twox_64_concat) EraIndex => (Balance, Balance, Balance); // (unbounding, total_claimed, initial_total_claimed)

		pub ClaimedUnbond get(fn claimed_unbond): double_map hasher(twox_64_concat) T::AccountId, hasher(twox_64_concat) EraIndex => Balance;
		pub TotalClaimedUnbonded get(fn total_claimed_unbonded): Balance;

		pub TotalBonded get(fn total_bonded): Balance;
		pub UnbondingToFree get(fn unbonding_to_free): Balance;
		pub FreeUnbonded get(fn free_unbonded): Balance;

		pub StakingPoolParams get(fn staking_pool_params) config(): Params;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;
		fn deposit_event() = default;

		/// The staking currency id(should be DOT in acala)
		const StakingCurrencyId: CurrencyId = T::StakingCurrencyId::get();

		/// The liquid currency id(should be LDOT in acala)
		const LiquidCurrencyId: CurrencyId = T::LiquidCurrencyId::get();

		/// The default exchange rate for liquid currency to staking currency.
		const DefaultExchangeRate: ExchangeRate = T::DefaultExchangeRate::get();

		/// The staking pool's module id, keep all staking currency belong to Homa protocol.
		const ModuleId: ModuleId = T::ModuleId::get();

		/// The sub account indexs of parachain to vault assets of Homa protocol in Polkadot.
		const PoolAccountIndexes: Vec<u32> = T::PoolAccountIndexes::get();

		#[weight = (10_000, DispatchClass::Operational)]
		#[transactional]
		pub fn set_staking_pool_params(
			origin,
			target_max_free_unbonded_ratio: ChangeRatio,
			target_min_free_unbonded_ratio: ChangeRatio,
			target_unbonding_to_free_ratio: ChangeRatio,
			unbonding_to_free_adjustment: ChangeRate,
			base_fee_rate: ChangeRate,
		) {
			T::UpdateOrigin::ensure_origin(origin)?;
			StakingPoolParams::try_mutate(|params| -> DispatchResult {
				if let Change::NewValue(update) = target_max_free_unbonded_ratio {
					params.target_max_free_unbonded_ratio = update;
				}
				if let Change::NewValue(update) = target_min_free_unbonded_ratio {
					params.target_min_free_unbonded_ratio = update;
				}
				if let Change::NewValue(update) = target_unbonding_to_free_ratio {
					params.target_unbonding_to_free_ratio = update;
				}
				if let Change::NewValue(update) = unbonding_to_free_adjustment {
					params.unbonding_to_free_adjustment = update;
				}
				if let Change::NewValue(update) = base_fee_rate {
					params.base_fee_rate = update;
				}

				ensure!(params.target_min_free_unbonded_ratio < params.target_max_free_unbonded_ratio, Error::<T>::InvalidConfig);
				Ok(())
			})?;
		}
	}
}

/// Impl helper for managing assets distributed on multiple sub accounts.
impl<T: Trait> Module<T> {
	/// Pass the sorted list, pick the first item
	pub fn distribute_increment(amount_list: Vec<(u32, Balance)>, increment: Balance) -> Vec<(u32, Balance)> {
		if amount_list.len().is_zero() {
			vec![]
		} else {
			vec![(amount_list[0].0, increment)]
		}
	}

	/// Pass the sorted list, consume available by order.
	pub fn distribute_decrement(amount_list: Vec<(u32, Balance)>, decrement: Balance) -> Vec<(u32, Balance)> {
		let mut distribution: Vec<(u32, Balance)> = vec![];
		let mut remain_decrement = decrement;

		for (sub_account_index, available) in amount_list {
			if remain_decrement.is_zero() {
				break;
			}
			distribution.push((sub_account_index, sp_std::cmp::min(available, remain_decrement)));
			remain_decrement = remain_decrement.saturating_sub(available);
		}

		distribution
	}

	pub fn bond_extra(amount: Balance) -> DispatchResult {
		let sub_accounts = T::PoolAccountIndexes::get();
		let mut current_available = sub_accounts
			.iter()
			.map(|account_index| {
				let staking_ledger = T::Bridge::staking_ledger(*account_index);
				let free = T::Bridge::balance(*account_index).saturating_sub(staking_ledger.total);
				(staking_ledger.active, *account_index, free)
			})
			.collect::<Vec<_>>();

		// Sort by bonded amount in ascending order
		current_available.sort_by(|a, b| a.0.cmp(&b.0));
		let current_available = current_available
			.iter()
			.map(|(_, account_index, free)| (*account_index, *free))
			.collect::<Vec<_>>();
		let distribution = Self::distribute_decrement(current_available, amount);

		for (account_index, val) in distribution {
			T::Bridge::bond_extra(account_index, val)?;
		}

		Ok(())
	}

	pub fn unbond(amount: Balance) -> DispatchResult {
		let sub_accounts = T::PoolAccountIndexes::get();
		let mut current_bonded = sub_accounts
			.iter()
			.map(|account_index| (*account_index, T::Bridge::staking_ledger(*account_index).active))
			.collect::<Vec<_>>();

		// Sort by bonded amount in descending order
		current_bonded.sort_by(|a, b| b.1.cmp(&a.1));
		let distribution = Self::distribute_decrement(current_bonded, amount);

		for (account_index, val) in distribution {
			T::Bridge::unbond(account_index, val)?;
		}

		Ok(())
	}

	pub fn receive_from_bridge(to: &T::AccountId, amount: Balance) -> DispatchResult {
		let sub_accounts = T::PoolAccountIndexes::get();
		let mut current_available = sub_accounts
			.iter()
			.map(|account_index| {
				let ledger = T::Bridge::staking_ledger(*account_index);
				let free = T::Bridge::balance(*account_index).saturating_sub(ledger.total);
				(ledger.active, *account_index, free)
			})
			.collect::<Vec<_>>();

		// Sort by bonded amount in descending order
		current_available.sort_by(|a, b| b.0.cmp(&a.0));
		let current_available = current_available
			.iter()
			.map(|(_, account_index, free)| (*account_index, *free))
			.collect::<Vec<_>>();
		let distribution = Self::distribute_decrement(current_available, amount);

		for (account_index, val) in distribution {
			T::Bridge::receive_from_bridge(account_index, to, val)?;
		}

		Ok(())
	}

	pub fn transfer_to_bridge(from: &T::AccountId, amount: Balance) -> DispatchResult {
		let sub_accounts = T::PoolAccountIndexes::get();
		let mut current_balance = sub_accounts
			.iter()
			.map(|account_index| (*account_index, T::Bridge::staking_ledger(*account_index).active))
			.collect::<Vec<_>>();

		// Sort by bonded amount in ascending order
		current_balance.sort_by(|a, b| a.1.cmp(&b.1));
		let distribution = Self::distribute_increment(current_balance, amount);

		for (account_index, val) in distribution.iter() {
			T::Bridge::transfer_to_bridge(*account_index, from, *val)?;
		}

		Ok(())
	}

	pub fn withdraw_unbonded() {
		for sub_account_index in T::PoolAccountIndexes::get() {
			T::Bridge::withdraw_unbonded(sub_account_index);
		}
	}

	pub fn payout_nominator() {
		for sub_account_index in T::PoolAccountIndexes::get() {
			T::Bridge::payout_nominator(sub_account_index);
		}
	}

	pub fn nominate(targets: Vec<PolkadotAccountIdOf<T>>) {
		for sub_account_index in T::PoolAccountIndexes::get() {
			T::Bridge::nominate(sub_account_index, targets.clone());
		}
	}

	/// Aggregate ledger of all sub accounts
	pub fn staking_ledger() -> PolkadotStakingLedger<Balance, EraIndex> {
		let mut active: Balance = Zero::zero();
		let mut total: Balance = Zero::zero();

		let mut accumulated_unlocking: Vec<PolkadotUnlockChunk<Balance, EraIndex>> = vec![];

		for sub_account_index in T::PoolAccountIndexes::get() {
			let ledger = T::Bridge::staking_ledger(sub_account_index);
			active = active.saturating_add(ledger.active);
			total = total.saturating_add(ledger.total);

			for chunk in ledger.unlocking {
				let mut find: bool = false;
				for (index, existd_chunk) in accumulated_unlocking.iter().enumerate() {
					if chunk.era == existd_chunk.era {
						accumulated_unlocking[index].value = existd_chunk.value.saturating_add(chunk.value);
						find = true;
						break;
					}
				}
				if !find {
					accumulated_unlocking.push(chunk.clone());
				}
			}
		}

		// sort list
		accumulated_unlocking.sort_by(|a, b| a.era.cmp(&b.era));

		PolkadotStakingLedger::<Balance, EraIndex> {
			total,
			active,
			unlocking: accumulated_unlocking,
		}
	}

	/// Aggregate balance of all sub accounts
	pub fn balance() -> Balance {
		let mut total: Balance = Zero::zero();
		for sub_account_index in T::PoolAccountIndexes::get() {
			total = total.saturating_add(T::Bridge::balance(sub_account_index));
		}
		total
	}
}

impl<T: Trait> Module<T> {
	/// Module account id
	pub fn account_id() -> T::AccountId {
		T::ModuleId::get().into_account()
	}

	/// It represent how much bonded DOT is belong to LDOT holders
	/// use it in operation checks
	pub fn get_communal_bonded() -> Balance {
		let (unbond_next_era, _) = Self::next_era_unbond();
		Self::total_bonded().saturating_sub(unbond_next_era)
	}

	/// It represent how much bonded DOT(include bonded, unbonded, unbonding) is
	/// belong to LDOT holders use it in exchange rate calculation
	pub fn get_total_communal_balance() -> Balance {
		Self::get_communal_bonded()
			.saturating_add(Self::free_unbonded())
			.saturating_add(Self::unbonding_to_free())
	}

	/// Percentage of free unbonded pool in total communal
	pub fn get_free_unbonded_ratio() -> Ratio {
		Ratio::checked_from_rational(Self::free_unbonded(), Self::get_total_communal_balance()).unwrap_or_default()
	}

	/// Percentage of total unbonding to free in total communal
	pub fn get_unbonding_to_free_ratio() -> Ratio {
		Ratio::checked_from_rational(Self::unbonding_to_free(), Self::get_total_communal_balance()).unwrap_or_default()
	}

	/// Percentage of total communal bonded in total communal
	pub fn get_communal_bonded_ratio() -> Ratio {
		Ratio::checked_from_rational(Self::get_communal_bonded(), Self::get_total_communal_balance())
			.unwrap_or_default()
	}

	/// liquid currency / staking currency  = total communal staking currency /
	/// total supply of liquid currency
	pub fn liquid_exchange_rate() -> ExchangeRate {
		let total_communal_staking_amount = Self::get_total_communal_balance();

		if !total_communal_staking_amount.is_zero() {
			let total_liquid_amount = T::Currency::total_issuance(T::LiquidCurrencyId::get());
			ExchangeRate::checked_from_rational(total_communal_staking_amount, total_liquid_amount)
				.unwrap_or_else(T::DefaultExchangeRate::get)
		} else {
			T::DefaultExchangeRate::get()
		}
	}

	pub fn get_available_unbonded(who: &T::AccountId) -> Balance {
		let current_era = Self::current_era();
		ClaimedUnbond::<T>::iter_prefix(who)
			.filter(|(era_index, _)| era_index <= &current_era)
			.fold(Zero::zero(), |available_unbonded, (_, claimed)| {
				available_unbonded.saturating_add(claimed)
			})
	}

	pub fn bond_to_bridge(amount: Balance) -> DispatchResult {
		if amount.is_zero() {
			return Ok(());
		}

		Self::transfer_to_bridge(&Self::account_id(), amount)?;
		Self::bond_extra(amount)?;

		FreeUnbonded::mutate(|free_unbonded| {
			*free_unbonded = free_unbonded.saturating_sub(amount);
		});
		TotalBonded::mutate(|total_bonded| {
			*total_bonded = total_bonded.saturating_add(amount);
		});

		Ok(())
	}

	pub fn unbond_from_bridge(era: EraIndex) {
		let (total_to_unbond, claimed_to_unbond) = Self::next_era_unbond();
		let bonding_duration = <<T as Trait>::Bridge as PolkadotBridgeType<_, _>>::BondingDuration::get();
		let unbonded_era_index = era.saturating_add(bonding_duration);

		if !total_to_unbond.is_zero() && Self::unbond(total_to_unbond).is_ok() {
			NextEraUnbond::kill();
			TotalBonded::mutate(|bonded| *bonded = bonded.saturating_sub(total_to_unbond));
			Unbonding::insert(
				unbonded_era_index,
				(total_to_unbond, claimed_to_unbond, claimed_to_unbond),
			);
			UnbondingToFree::mutate(|unbonding| {
				*unbonding = unbonding.saturating_add(total_to_unbond.saturating_sub(claimed_to_unbond))
			});
		}
	}

	pub fn rebalance(era: EraIndex) {
		// #1: bridge withdraw unbonded and withdraw payout
		Self::withdraw_unbonded();

		// TODO: record the balances of bridge before and after do payout_nominator,
		// and oncommision to homa treasury according to RewardFeeRatio
		Self::payout_nominator();

		// #2: update staking pool by bridge ledger
		// TODO: adjust the amount of this era unbond by the slash situation in last era
		let bridge_ledger = Self::staking_ledger();
		TotalBonded::put(bridge_ledger.active);

		// #3: withdraw available from bridge ledger and update unbonded at this era
		let bridge_available = Self::balance().saturating_sub(bridge_ledger.total);
		if Self::receive_from_bridge(&Self::account_id(), bridge_available).is_ok() {
			let (total_unbonded, claimed_unbonded, _) = Self::unbonding(era);
			let claimed_unbonded_added = bridge_available.min(claimed_unbonded);
			let free_unbonded_added = bridge_available.saturating_sub(claimed_unbonded_added);
			if !claimed_unbonded_added.is_zero() {
				TotalClaimedUnbonded::mutate(|balance| *balance = balance.saturating_add(claimed_unbonded_added));
			}
			if !free_unbonded_added.is_zero() {
				FreeUnbonded::mutate(|balance| *balance = balance.saturating_add(free_unbonded_added));
			}
			UnbondingToFree::mutate(|balance| {
				*balance = balance.saturating_sub(total_unbonded.saturating_sub(claimed_unbonded))
			});
			Unbonding::remove(era);
		}

		// #4: according to the pool adjustment params, bond and unbond at this era
		let staking_pool_params = Self::staking_pool_params();
		let bond_rate =
			Self::get_free_unbonded_ratio().saturating_sub(staking_pool_params.target_max_free_unbonded_ratio);
		let bond_amount = bond_rate
			.saturating_mul_int(Self::get_total_communal_balance())
			.min(Self::free_unbonded());

		let unbond_rate = staking_pool_params
			.target_unbonding_to_free_ratio
			.saturating_sub(Self::get_unbonding_to_free_ratio())
			.min(staking_pool_params.unbonding_to_free_adjustment);
		let unbond_amount = unbond_rate
			.saturating_mul_int(Self::get_total_communal_balance())
			.min(Self::get_communal_bonded());

		if !bond_amount.is_zero() {
			// bound more amount for staking. if it failed, just that added amount did not
			// succeed and it should not affect the process. so ignore result to continue.
			let _ = Self::bond_to_bridge(bond_amount);
		}

		if !unbond_amount.is_zero() {
			NextEraUnbond::mutate(|(unbond, _)| *unbond = unbond.saturating_add(unbond_amount));
		}

		// #5: unbond from bridge
		Self::unbond_from_bridge(era);

		// #6: nominate
		Self::nominate(T::Nominees::nominees());
	}
}

impl<T: Trait> OnNewEra<EraIndex> for Module<T> {
	fn on_new_era(new_era: EraIndex) {
		CurrentEra::put(new_era);
		Self::rebalance(new_era);
	}
}

impl<T: Trait> HomaProtocol<T::AccountId, Balance, EraIndex> for Module<T> {
	type Balance = Balance;

	/// Ensure atomic.
	#[transactional]
	fn mint(who: &T::AccountId, amount: Self::Balance) -> sp_std::result::Result<Self::Balance, DispatchError> {
		if amount.is_zero() {
			return Ok(Zero::zero());
		}

		// transfer staking currency to staking pool
		T::Currency::transfer(T::StakingCurrencyId::get(), who, &Self::account_id(), amount)?;
		FreeUnbonded::mutate(|free| {
			*free = free.saturating_add(amount);
		});

		// issue liquid currency to who
		let liquid_amount_to_issue = Self::liquid_exchange_rate()
			.reciprocal()
			.unwrap_or_default()
			.checked_mul_int(amount)
			.ok_or(Error::<T>::Overflow)?;
		T::Currency::deposit(T::LiquidCurrencyId::get(), who, liquid_amount_to_issue)?;

		<Module<T>>::deposit_event(RawEvent::MintLiquid(who.clone(), amount, liquid_amount_to_issue));
		Ok(liquid_amount_to_issue)
	}

	/// Ensure atomic.
	#[transactional]
	fn redeem_by_unbond(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		let mut liquid_amount_to_redeem = amount;
		let liquid_exchange_rate = Self::liquid_exchange_rate();
		let mut staking_amount_to_unbond = liquid_exchange_rate
			.checked_mul_int(liquid_amount_to_redeem)
			.ok_or(Error::<T>::Overflow)?;
		let communal_bonded_staking_amount = Self::get_communal_bonded();

		if !staking_amount_to_unbond.is_zero() && !communal_bonded_staking_amount.is_zero() {
			// communal_bonded_staking_amount is not enough, re-calculate
			if staking_amount_to_unbond > communal_bonded_staking_amount {
				liquid_amount_to_redeem = liquid_exchange_rate
					.reciprocal()
					.unwrap_or_default()
					.saturating_mul_int(communal_bonded_staking_amount);
				staking_amount_to_unbond = communal_bonded_staking_amount;
			}

			// burn liquid currency
			T::Currency::withdraw(T::LiquidCurrencyId::get(), who, liquid_amount_to_redeem)
				.map_err(|_| Error::<T>::LiquidCurrencyNotEnough)?;

			// start unbond at next era, and the unbond become unbonded after bonding
			// duration
			let unbonded_era_index = Self::current_era()
				.checked_add(EraIndex::one())
				.and_then(|n| n.checked_add(<<T as Trait>::Bridge as PolkadotBridgeType<_, _>>::BondingDuration::get()))
				.ok_or(Error::<T>::Overflow)?;

			NextEraUnbond::mutate(|(unbond, claimed)| {
				*unbond = unbond.saturating_add(staking_amount_to_unbond);
				*claimed = claimed.saturating_add(staking_amount_to_unbond);
			});
			ClaimedUnbond::<T>::mutate(who, unbonded_era_index, |balance| {
				*balance = balance.saturating_add(staking_amount_to_unbond);
			});

			<Module<T>>::deposit_event(RawEvent::RedeemByUnbond(
				who.clone(),
				liquid_amount_to_redeem,
				staking_amount_to_unbond,
			));
		}

		Ok(())
	}

	/// Ensure atomic.
	#[transactional]
	fn redeem_by_free_unbonded(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		let mut redeem_liquid_amount = amount;
		let liquid_exchange_rate = Self::liquid_exchange_rate();
		let mut demand_staking_amount = liquid_exchange_rate
			.checked_mul_int(redeem_liquid_amount)
			.ok_or(Error::<T>::Overflow)?;

		let staking_pool_params = Self::staking_pool_params();
		let available_free_unbonded = Self::free_unbonded().saturating_sub(
			staking_pool_params
				.target_min_free_unbonded_ratio
				.saturating_mul_int(Self::get_total_communal_balance()),
		);

		if !demand_staking_amount.is_zero() && !available_free_unbonded.is_zero() {
			// if available_free_unbonded is not enough, need re-calculate
			if demand_staking_amount > available_free_unbonded {
				let ratio = Ratio::checked_from_rational(available_free_unbonded, demand_staking_amount)
					.expect("demand_staking_amount is not zero; qed");
				redeem_liquid_amount = ratio.saturating_mul_int(redeem_liquid_amount);
				demand_staking_amount = available_free_unbonded;
			}

			let current_free_unbonded_ratio = Self::get_free_unbonded_ratio();
			let remain_available_percent = current_free_unbonded_ratio
				.saturating_sub(staking_pool_params.target_min_free_unbonded_ratio)
				.checked_div(
					&sp_std::cmp::max(
						staking_pool_params.target_max_free_unbonded_ratio,
						current_free_unbonded_ratio,
					)
					.saturating_sub(staking_pool_params.target_min_free_unbonded_ratio),
				)
				.expect("shouldn't panic expect the fee config is incorrect; qed");
			let fee_in_staking = T::FeeModel::get_fee(
				remain_available_percent,
				available_free_unbonded,
				demand_staking_amount,
				staking_pool_params.base_fee_rate,
			)
			.ok_or(Error::<T>::GetFeeFailed)?;

			let retrieved_staking_amount = demand_staking_amount.saturating_sub(fee_in_staking);

			T::Currency::withdraw(T::LiquidCurrencyId::get(), who, redeem_liquid_amount)
				.map_err(|_| Error::<T>::LiquidCurrencyNotEnough)?;
			T::Currency::transfer(
				T::StakingCurrencyId::get(),
				&Self::account_id(),
				who,
				retrieved_staking_amount,
			)?;

			FreeUnbonded::mutate(|free_unbonded| {
				*free_unbonded = free_unbonded.saturating_sub(retrieved_staking_amount);
			});

			<Module<T>>::deposit_event(RawEvent::RedeemByFreeUnbonded(
				who.clone(),
				redeem_liquid_amount,
				retrieved_staking_amount,
				fee_in_staking,
			));
		}

		Ok(())
	}

	/// Ensure atomic.
	#[transactional]
	fn redeem_by_claim_unbonding(who: &T::AccountId, amount: Self::Balance, target_era: EraIndex) -> DispatchResult {
		let current_era = Self::current_era();
		let bonding_duration = <<T as Trait>::Bridge as PolkadotBridgeType<_, _>>::BondingDuration::get();
		ensure!(
			target_era > current_era && target_era <= current_era + bonding_duration,
			Error::<T>::InvalidEra,
		);

		let mut redeem_liquid_amount = amount;
		let liquid_exchange_rate = Self::liquid_exchange_rate();
		let mut demand_staking_amount = liquid_exchange_rate
			.checked_mul_int(redeem_liquid_amount)
			.ok_or(Error::<T>::Overflow)?;
		let (unbonding, claimed_unbonding, initial_claimed_unbonding) = Self::unbonding(target_era);
		let staking_pool_params = Self::staking_pool_params();

		let initial_unclaimed = unbonding.saturating_sub(initial_claimed_unbonding);
		let unclaimed = unbonding.saturating_sub(claimed_unbonding);

		let available_unclaimed_unbonding = unclaimed.saturating_sub(
			staking_pool_params
				.target_min_free_unbonded_ratio
				.saturating_mul_int(initial_unclaimed),
		);

		if !demand_staking_amount.is_zero() && !available_unclaimed_unbonding.is_zero() {
			// if available_unclaimed_unbonding is not enough, need re-calculate
			if demand_staking_amount > available_unclaimed_unbonding {
				let ratio = Ratio::checked_from_rational(available_unclaimed_unbonding, demand_staking_amount)
					.expect("staking_amount_to_claim is not zero; qed");
				redeem_liquid_amount = ratio.saturating_mul_int(redeem_liquid_amount);
				demand_staking_amount = available_unclaimed_unbonding;
			}

			let current_unclaimed_ratio = Ratio::checked_from_rational(unclaimed, initial_unclaimed)
				.expect("if available_unclaimed_unbonding is not zero, initial_unclaimed must not be zero; qed");

			let remain_available_percent = current_unclaimed_ratio
				.saturating_sub(staking_pool_params.target_min_free_unbonded_ratio)
				.checked_div(
					&sp_std::cmp::max(
						staking_pool_params.target_max_free_unbonded_ratio,
						current_unclaimed_ratio,
					)
					.saturating_sub(staking_pool_params.target_min_free_unbonded_ratio),
				)
				.unwrap_or_default();

			let fee_in_staking = T::FeeModel::get_fee(
				remain_available_percent,
				available_unclaimed_unbonding,
				demand_staking_amount,
				staking_pool_params.base_fee_rate,
			)
			.ok_or(Error::<T>::GetFeeFailed)?;

			let claimed_staking_amount = demand_staking_amount.saturating_sub(fee_in_staking);

			T::Currency::withdraw(T::LiquidCurrencyId::get(), who, redeem_liquid_amount)
				.map_err(|_| Error::<T>::LiquidCurrencyNotEnough)?;

			ClaimedUnbond::<T>::mutate(who, target_era, |claimed_unbond| {
				*claimed_unbond = claimed_unbond.saturating_add(claimed_staking_amount);
			});
			Unbonding::mutate(target_era, |(_, claimed_unbonding, _)| {
				*claimed_unbonding = claimed_unbonding.saturating_add(claimed_staking_amount);
			});
			UnbondingToFree::mutate(|unbonding_to_free| {
				*unbonding_to_free = unbonding_to_free.saturating_sub(claimed_staking_amount);
			});

			<Module<T>>::deposit_event(RawEvent::RedeemByClaimUnbonding(
				who.clone(),
				target_era,
				redeem_liquid_amount,
				claimed_staking_amount,
				fee_in_staking,
			));
		}

		Ok(())
	}

	/// Ensure atomic.
	#[transactional]
	fn withdraw_redemption(who: &T::AccountId) -> sp_std::result::Result<Self::Balance, DispatchError> {
		let current_era = Self::current_era();
		let staking_currency_id = T::StakingCurrencyId::get();
		let mut withdrawn_amount: Balance = Zero::zero();

		ClaimedUnbond::<T>::iter_prefix(who)
			.filter(|(era_index, _)| era_index <= &current_era)
			.for_each(|(era_index, claimed)| {
				withdrawn_amount = withdrawn_amount.saturating_add(claimed);
				ClaimedUnbond::<T>::remove(who, era_index);
			});

		T::Currency::transfer(staking_currency_id, &Self::account_id(), who, withdrawn_amount)?;
		TotalClaimedUnbonded::mutate(|balance| *balance = balance.saturating_sub(withdrawn_amount));
		Ok(withdrawn_amount)
	}
}
