#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get};
use orml_traits::BasicCurrency;
use rstd::prelude::*;
use sp_runtime::{
	traits::{AccountIdConversion, Saturating, UniqueSaturatedInto, Zero},
	DispatchResult, ModuleId,
};
use support::{
	EraIndex, ExchangeRate, NomineesProvider, OnCommission, OnNewEra, PolkadotBridge, PolkadotBridgeCall,
	PolkadotBridgeState, PolkadotBridgeType, Rate, Ratio,
};
use system::{self as system};

mod mock;
mod tests;

const MODULE_ID: ModuleId = ModuleId(*b"aca/stkp");

type StakingBalanceOf<T> = <<T as Trait>::StakingCurrency as BasicCurrency<<T as system::Trait>::AccountId>>::Balance;
type LiquidBalanceOf<T> = <<T as Trait>::LiquidCurrency as BasicCurrency<<T as system::Trait>::AccountId>>::Balance;
type PolkadotAccountIdOf<T> =
	<<T as Trait>::Bridge as PolkadotBridgeType<<T as system::Trait>::BlockNumber>>::PolkadotAccountId;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type StakingCurrency: BasicCurrency<Self::AccountId>;
	type LiquidCurrency: BasicCurrency<Self::AccountId>;
	type Nominees: NomineesProvider<PolkadotAccountIdOf<Self>>;
	type OnCommission: OnCommission<LiquidBalanceOf<Self>>;
	type Bridge: PolkadotBridge<Self::BlockNumber, StakingBalanceOf<Self>, Self::AccountId>;
	type MaxBondRatio: Get<Ratio>;
	type MinBondRatio: Get<Ratio>;
	type MaxClaimFee: Get<Rate>;
	type DefaultExchangeRate: Get<ExchangeRate>;
}

decl_event!(
	pub enum Event<T>
	where
		<T as system::Trait>::AccountId,
		StakingBalance = StakingBalanceOf<T>,
		LiquidBalance = LiquidBalanceOf<T>,
	{
		BondAndMint(AccountId, StakingBalance, LiquidBalance),
		RedeemByUnbond(AccountId, LiquidBalance),
		RedeemByFreePool(AccountId, LiquidBalance, LiquidBalance, StakingBalance),
		RedeemByClaimUnbonding(AccountId, EraIndex, LiquidBalance, LiquidBalance, StakingBalance),
	}
);

decl_error! {
	/// Error for staking pool module.
	pub enum Error for Module<T: Trait> {
		StakingCurrencyNotEnough,
		LiquidCurrencyNotEnough,
		InvalidEra,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as StakingPool {
		pub CurrentEra get(current_era): EraIndex;

		pub NextEraUnbond get(next_era_unbond): (StakingBalanceOf<T>, StakingBalanceOf<T>);
		pub Unbonding get(unbonding): map hasher(twox_64_concat) EraIndex => (StakingBalanceOf<T>, StakingBalanceOf<T>); // (value, claimed), value - claimed = unbond to free pool
		pub ClaimedUnbond get(claimed_unbond): double_map hasher(twox_64_concat) EraIndex, hasher(twox_64_concat) T::AccountId => StakingBalanceOf<T>;

		pub UnbondingToFree get(unbonding_to_free): StakingBalanceOf<T>;
		pub TotalBonded get(total_bonded): StakingBalanceOf<T>;
		pub FreePool get(free_pool): StakingBalanceOf<T>;
		pub TotalClaimedUnbonded get(total_claimed_unbonded): StakingBalanceOf<T>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		const MaxBondRatio: Ratio = T::MaxBondRatio::get();
		const MinBondRatio: Ratio = T::MinBondRatio::get();
		const MaxClaimFee: Rate = T::MaxClaimFee::get();
		const DefaultExchangeRate: ExchangeRate = T::DefaultExchangeRate::get();
	}
}

impl<T: Trait> Module<T> {
	pub fn account_id() -> T::AccountId {
		MODULE_ID.into_account()
	}

	// the total unclaimed balance is all DOT that belong to all existing LDOT
	pub fn get_total_unclaimed_balance() -> StakingBalanceOf<T> {
		Self::total_bonded()
			.saturating_add(Self::free_pool())
			.saturating_add(Self::unbonding_to_free())
			.saturating_sub(Self::next_era_unbond().1)
	}

	// bonded_ratio = total_bonded / total_balance
	pub fn get_bonded_ratio() -> Ratio {
		Ratio::from_rational(Self::total_bonded(), Self::get_total_unclaimed_balance())
	}

	// LDOT/DOT = total DOT belong to Homa / total supply of LDOT
	pub fn liquid_exchange_rate() -> ExchangeRate {
		let total_dot_amount = Self::get_total_unclaimed_balance();
		let total_ldot_amount: u128 = T::LiquidCurrency::total_issuance().unique_saturated_into();
		let total_ldot_amount: StakingBalanceOf<T> = total_ldot_amount.unique_saturated_into();

		if !total_dot_amount.is_zero() && !total_ldot_amount.is_zero() {
			ExchangeRate::from_rational(total_dot_amount, total_ldot_amount)
		} else {
			T::DefaultExchangeRate::get()
		}
	}

	// TODO: iterator all expired era, instead of specific era
	pub fn withdraw_unbonded(who: &T::AccountId, era: EraIndex) -> DispatchResult {
		ensure!(era <= Self::current_era(), Error::<T>::InvalidEra);
		let unbonded = Self::claimed_unbond(era, &who);
		if !unbonded.is_zero() {
			T::StakingCurrency::transfer(&Self::account_id(), who, unbonded)?;
			<TotalClaimedUnbonded<T>>::mutate(|balance| *balance -= unbonded);
			<ClaimedUnbond<T>>::remove(era, who);
		}
		Ok(())
	}

	pub fn bond(who: &T::AccountId, amount: StakingBalanceOf<T>) -> DispatchResult {
		// bond dot
		T::StakingCurrency::ensure_can_withdraw(who, amount).map_err(|_| Error::<T>::StakingCurrencyNotEnough)?;
		T::Bridge::transfer_to_bridge(who, amount);
		T::Bridge::bond_extra(amount);
		<TotalBonded<T>>::mutate(|bonded| *bonded += amount);

		// issue ldot to who
		let ldot_amount: u128 = ExchangeRate::from_natural(1)
			.checked_div(&Self::liquid_exchange_rate())
			.unwrap_or_default()
			.saturating_mul_int(&Self::get_total_unclaimed_balance())
			.unique_saturated_into();
		let ldot_amount: LiquidBalanceOf<T> = ldot_amount.unique_saturated_into();
		T::LiquidCurrency::deposit(who, ldot_amount)?;

		<Module<T>>::deposit_event(RawEvent::BondAndMint(who.clone(), amount, ldot_amount));
		Ok(())
	}

	pub fn redeem_by_unbond(who: &T::AccountId, amount: LiquidBalanceOf<T>) -> DispatchResult {
		let mut ldot_to_redeem = amount;
		let liquid_exchange_rate = Self::liquid_exchange_rate();
		let unbond_value: u128 = liquid_exchange_rate
			.saturating_mul_int(&ldot_to_redeem)
			.unique_saturated_into();
		let mut unbond_amount: StakingBalanceOf<T> = unbond_value.unique_saturated_into();
		let total_bonded = Self::total_bonded();

		if !unbond_amount.is_zero() && !total_bonded.is_zero() {
			if unbond_amount > total_bonded {
				// total_bonded is not enough, calculate actual redeem ldot
				let new_redeem_amount: u128 = ExchangeRate::from_natural(1)
					.checked_div(&liquid_exchange_rate)
					.unwrap_or_default()
					.saturating_mul_int(&total_bonded)
					.unique_saturated_into();
				ldot_to_redeem = new_redeem_amount.unique_saturated_into();
				unbond_amount = total_bonded;
			}

			// burn who's ldot
			T::LiquidCurrency::ensure_can_withdraw(who, ldot_to_redeem)
				.map_err(|_| Error::<T>::LiquidCurrencyNotEnough)?;
			T::LiquidCurrency::withdraw(who, ldot_to_redeem).expect("never failed after balance check");

			// start unbond at next era, and the unbond become unbonded after bonding duration
			let unbonded_era_index = Self::current_era()
				+ 1 + <<T as Trait>::Bridge as PolkadotBridgeType<
				<T as system::Trait>::BlockNumber,
			>>::BondingDuration::get();
			<NextEraUnbond<T>>::mutate(|(unbond, claimed)| {
				*unbond += unbond_amount;
				*claimed += unbond_amount;
			});
			<ClaimedUnbond<T>>::mutate(unbonded_era_index, who, |balance| *balance += unbond_amount);
			<Module<T>>::deposit_event(RawEvent::RedeemByUnbond(who.clone(), ldot_to_redeem));
		}

		Ok(())
	}

	pub fn claim_period_percent(era: EraIndex) -> Ratio {
		Ratio::from_rational(
			era.saturating_sub(Self::current_era()),
			<<T as Trait>::Bridge as PolkadotBridgeType<<T as system::Trait>::BlockNumber>>::BondingDuration::get(),
		)
	}

	pub fn calculate_claim_fee(amount: LiquidBalanceOf<T>, era: EraIndex) -> LiquidBalanceOf<T> {
		Ratio::from_natural(1)
			.saturating_sub(Self::claim_period_percent(era))
			.saturating_mul(T::MaxClaimFee::get())
			.saturating_mul_int(&amount)
	}

	pub fn redeem_by_free_pool(who: &T::AccountId, amount: LiquidBalanceOf<T>) -> DispatchResult {
		let current_era = Self::current_era();
		let mut total_deduct = amount;
		let mut fee = Self::calculate_claim_fee(total_deduct, current_era);
		let mut ldot_to_redeem = total_deduct.saturating_sub(fee);
		let liquid_exchange_rate = Self::liquid_exchange_rate();
		let unbond_value: u128 = liquid_exchange_rate
			.saturating_mul_int(&ldot_to_redeem)
			.unique_saturated_into();
		let mut unbond_amount: StakingBalanceOf<T> = unbond_value.unique_saturated_into();
		let free_pool = Self::free_pool();

		if !unbond_amount.is_zero() && !free_pool.is_zero() {
			if unbond_amount > free_pool {
				// free_pool is not enough, re-calculate actual redeem ldot
				let new_ldot_to_redeem: u128 = ExchangeRate::from_natural(1)
					.checked_div(&liquid_exchange_rate)
					.unwrap_or_default()
					.saturating_mul_int(&free_pool)
					.unique_saturated_into();
				let new_ldot_to_redeem: LiquidBalanceOf<T> = new_ldot_to_redeem.unique_saturated_into();

				// re-assign
				fee = Ratio::from_rational(new_ldot_to_redeem, ldot_to_redeem).saturating_mul_int(&fee);
				ldot_to_redeem = new_ldot_to_redeem;
				unbond_amount = free_pool;
				total_deduct = fee + ldot_to_redeem;
			}

			T::LiquidCurrency::ensure_can_withdraw(who, total_deduct)
				.map_err(|_| Error::<T>::LiquidCurrencyNotEnough)?;
			T::StakingCurrency::transfer(&Self::account_id(), who, unbond_amount)?;
			T::LiquidCurrency::withdraw(who, total_deduct).expect("never failed after balance check");
			<FreePool<T>>::mutate(|balance| *balance -= unbond_amount);
			T::OnCommission::on_commission(fee);
			<Module<T>>::deposit_event(RawEvent::RedeemByFreePool(
				who.clone(),
				fee,
				ldot_to_redeem,
				unbond_amount,
			));
		}

		Ok(())
	}

	pub fn redeem_by_claim_unbonding(
		who: &T::AccountId,
		amount: LiquidBalanceOf<T>,
		target_era: EraIndex,
	) -> DispatchResult {
		let current_era = Self::current_era();
		let bonding_duration =
			<<T as Trait>::Bridge as PolkadotBridgeType<<T as system::Trait>::BlockNumber>>::BondingDuration::get();
		ensure!(
			target_era > current_era && target_era <= current_era + bonding_duration,
			Error::<T>::InvalidEra,
		);

		let mut total_deduct = amount;
		let mut fee = Self::calculate_claim_fee(total_deduct, target_era);
		let mut ldot_to_redeem = total_deduct.saturating_sub(fee);
		let liquid_exchange_rate = Self::liquid_exchange_rate();
		let unbond_value: u128 = liquid_exchange_rate
			.saturating_mul_int(&ldot_to_redeem)
			.unique_saturated_into();
		let mut unbond_amount: StakingBalanceOf<T> = unbond_value.unique_saturated_into();
		let target_era_unbonding = Self::unbonding(target_era);
		let target_era_unclaimed = target_era_unbonding.0.saturating_sub(target_era_unbonding.1);

		if !unbond_amount.is_zero() && !target_era_unclaimed.is_zero() {
			if unbond_amount > target_era_unclaimed {
				// target_era_unclaimed is not enough, re-calculate actual redeem ldot
				let new_ldot_to_redeem: u128 = ExchangeRate::from_natural(1)
					.checked_div(&liquid_exchange_rate)
					.unwrap_or_default()
					.saturating_mul_int(&target_era_unclaimed)
					.unique_saturated_into();
				let new_ldot_to_redeem: LiquidBalanceOf<T> = new_ldot_to_redeem.unique_saturated_into();

				// re-assign
				fee = Ratio::from_rational(new_ldot_to_redeem, ldot_to_redeem).saturating_mul_int(&fee);
				ldot_to_redeem = new_ldot_to_redeem;
				unbond_amount = target_era_unclaimed;
				total_deduct = fee + ldot_to_redeem;
			}

			T::LiquidCurrency::ensure_can_withdraw(who, total_deduct)
				.map_err(|_| Error::<T>::LiquidCurrencyNotEnough)?;
			T::LiquidCurrency::withdraw(who, total_deduct).expect("never failed after balance check");

			<ClaimedUnbond<T>>::mutate(target_era, who, |balance| *balance += unbond_amount);
			<Unbonding<T>>::mutate(target_era, |(_, claimed)| *claimed += unbond_amount);
			<UnbondingToFree<T>>::mutate(|balance| *balance = balance.saturating_sub(unbond_amount));
			T::OnCommission::on_commission(fee);
			<Module<T>>::deposit_event(RawEvent::RedeemByClaimUnbonding(
				who.clone(),
				target_era,
				fee,
				ldot_to_redeem,
				unbond_amount,
			));
		}

		Ok(())
	}

	pub fn unbond_and_update(era: EraIndex) {
		let (total_to_unbond, claimed_to_unbond) = <NextEraUnbond<T>>::take();
		let bonding_duration =
			<<T as Trait>::Bridge as PolkadotBridgeType<<T as system::Trait>::BlockNumber>>::BondingDuration::get();
		let unbonded_era_index = era + bonding_duration;

		if !total_to_unbond.is_zero() {
			T::Bridge::unbond(total_to_unbond);
			<Unbonding<T>>::insert(unbonded_era_index, (total_to_unbond, claimed_to_unbond));
			<UnbondingToFree<T>>::mutate(|unbonding| *unbonding += total_to_unbond - claimed_to_unbond);
		}
	}

	pub fn rebalance(era: EraIndex) {
		// #1: bridge withdraw unbonded and withdraw payout
		T::Bridge::withdraw_unbonded();
		T::Bridge::payout_nominator();

		// #2: update staking pool by bridge ledger
		let bridge_ledger = T::Bridge::ledger();
		<TotalBonded<T>>::put(bridge_ledger.active);

		// #3: withdraw available from bridge ledger
		let bridge_available = T::Bridge::balance().saturating_sub(bridge_ledger.total);
		T::Bridge::receive_from_bridge(&Self::account_id(), bridge_available);

		// #4: update unbonded
		let (total_unbonded, claimed_unbonded) = Self::unbonding(era);
		let claimed_unbonded_added = bridge_available.min(claimed_unbonded);
		let free_pool_added = bridge_available.saturating_sub(claimed_unbonded_added);
		if !claimed_unbonded_added.is_zero() {
			<TotalClaimedUnbonded<T>>::mutate(|balance| *balance += claimed_unbonded_added);
		}
		if !free_pool_added.is_zero() {
			<FreePool<T>>::mutate(|balance| *balance += free_pool_added);
		}
		<UnbondingToFree<T>>::mutate(|balance| *balance = balance.saturating_sub(total_unbonded - claimed_unbonded));
		<Unbonding<T>>::remove(era);

		// #5 TODO: adjust the amount user unbond at this era by the slash amount in last era

		// #6: according to bonded_ratio, decide to
		// bond extra amount to bridge or unbond system bonded to free pool at this era
		let bonded_ratio = Self::get_bonded_ratio();
		let max_bond_ratio = T::MaxBondRatio::get();
		let min_bond_ratio = T::MinBondRatio::get();
		let total_balance = Self::get_total_unclaimed_balance();
		if bonded_ratio > max_bond_ratio {
			// unbond some
			let unbond_to_free = bonded_ratio
				.saturating_sub(max_bond_ratio)
				.saturating_mul_int(&total_balance)
				.min(Self::total_bonded());

			if !unbond_to_free.is_zero() {
				<NextEraUnbond<T>>::mutate(|(unbond, _)| *unbond += unbond_to_free);
			}
		} else if bonded_ratio < min_bond_ratio {
			// bond more
			let bond_amount = min_bond_ratio
				.saturating_sub(bonded_ratio)
				.saturating_mul_int(&total_balance)
				.min(Self::free_pool());

			T::Bridge::transfer_to_bridge(&Self::account_id(), bond_amount);
			T::Bridge::bond_extra(bond_amount);
			<FreePool<T>>::mutate(|balance| *balance -= bond_amount);
		}

		// #7: unbond and update
		Self::unbond_and_update(era);
	}
}

impl<T: Trait> OnNewEra for Module<T> {
	fn on_new_era(new_era: EraIndex) {
		CurrentEra::put(new_era);

		// rebalance first
		Self::rebalance(new_era);

		// nominate
		T::Bridge::nominate(T::Nominees::nominees());
	}
}
