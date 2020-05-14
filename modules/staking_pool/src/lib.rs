#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get, IterableStorageDoubleMap};
use frame_system::{self as system};
use orml_traits::MultiCurrency;
use primitives::{Balance, CurrencyId, EraIndex};
use sp_runtime::{
	traits::{AccountIdConversion, One, Saturating, UniqueSaturatedInto, Zero},
	DispatchError, DispatchResult, ModuleId,
};
use sp_std::prelude::*;
use support::{
	ExchangeRate, HomaProtocol, NomineesProvider, OnCommission, OnNewEra, PolkadotBridge, PolkadotBridgeCall,
	PolkadotBridgeState, PolkadotBridgeType, Rate, Ratio,
};

mod mock;
mod tests;

const MODULE_ID: ModuleId = ModuleId(*b"aca/stkp");

type PolkadotAccountIdOf<T> =
	<<T as Trait>::Bridge as PolkadotBridgeType<<T as system::Trait>::BlockNumber, EraIndex>>::PolkadotAccountId;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;
	type StakingCurrencyId: Get<CurrencyId>;
	type LiquidCurrencyId: Get<CurrencyId>;
	type Nominees: NomineesProvider<PolkadotAccountIdOf<Self>>;
	type OnCommission: OnCommission<Balance, CurrencyId>;
	type Bridge: PolkadotBridge<Self::AccountId, Self::BlockNumber, Balance, EraIndex>;
	type MaxBondRatio: Get<Ratio>;
	type MinBondRatio: Get<Ratio>;
	type MaxClaimFee: Get<Rate>;
	type DefaultExchangeRate: Get<ExchangeRate>;
	type ClaimFeeReturnRatio: Get<Ratio>;

	// TODO: add RewardFeeRatio
}

decl_event!(
	pub enum Event<T>
	where
		<T as system::Trait>::AccountId,
		Balance = Balance,
	{
		BondAndMint(AccountId, Balance, Balance),
		RedeemByUnbond(AccountId, Balance),
		RedeemByFreeUnbonded(AccountId, Balance, Balance, Balance),
		RedeemByClaimUnbonding(AccountId, EraIndex, Balance, Balance, Balance),
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

		pub NextEraUnbond get(next_era_unbond): (Balance, Balance);
		pub Unbonding get(unbonding): map hasher(twox_64_concat) EraIndex => (Balance, Balance); // (value, claimed), value - claimed = unbond to free

		pub ClaimedUnbond get(claimed_unbond): double_map hasher(twox_64_concat) T::AccountId, hasher(twox_64_concat) EraIndex => Balance;
		pub TotalClaimedUnbonded get(total_claimed_unbonded): Balance;

		pub TotalBonded get(total_bonded): Balance;
		pub UnbondingToFree get(unbonding_to_free): Balance;
		pub FreeUnbonded get(free_unbonded): Balance;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;
		fn deposit_event() = default;

		const StakingCurrencyId: CurrencyId = T::StakingCurrencyId::get();
		const LiquidCurrencyId: CurrencyId = T::LiquidCurrencyId::get();
		const MaxBondRatio: Ratio = T::MaxBondRatio::get();
		const MinBondRatio: Ratio = T::MinBondRatio::get();
		const MaxClaimFee: Rate = T::MaxClaimFee::get();
		const DefaultExchangeRate: ExchangeRate = T::DefaultExchangeRate::get();
		const ClaimFeeReturnRatio: Ratio = T::ClaimFeeReturnRatio::get();
	}
}

impl<T: Trait> Module<T> {
	pub fn account_id() -> T::AccountId {
		MODULE_ID.into_account()
	}

	// it represent how much bonded DOT is belong to LDOT holders
	// use it in operation checks
	pub fn get_communal_bonded() -> Balance {
		Self::total_bonded().saturating_sub(Self::next_era_unbond().1)
	}

	// it represent how much bonded DOT(include bonded, unbonded, unbonding) is belong to LDOT holders
	// use it in exchange rate calculation
	pub fn get_total_communal_balance() -> Balance {
		Self::get_communal_bonded()
			.saturating_add(Self::free_unbonded())
			.saturating_add(Self::unbonding_to_free())
	}

	// communal_bonded_ratio = communal_bonded / total_communal_balance
	pub fn get_communal_bonded_ratio() -> Ratio {
		Ratio::from_rational(Self::get_communal_bonded(), Self::get_total_communal_balance())
	}

	// LDOT/DOT = total communal DOT / total supply of LDOT
	pub fn liquid_exchange_rate() -> ExchangeRate {
		let total_dot_amount = Self::get_total_communal_balance();
		let total_ldot_amount = T::Currency::total_issuance(T::LiquidCurrencyId::get());

		if !total_dot_amount.is_zero() && !total_ldot_amount.is_zero() {
			ExchangeRate::from_rational(total_dot_amount, total_ldot_amount)
		} else {
			T::DefaultExchangeRate::get()
		}
	}

	pub fn get_available_unbonded(who: &T::AccountId) -> Balance {
		let current_era = Self::current_era();
		let claimed_unbond = <ClaimedUnbond<T>>::iter(who).collect::<Vec<(EraIndex, Balance)>>();
		let mut available_unbonded: Balance = Zero::zero();

		for (era_index, claimed) in claimed_unbond {
			if era_index <= current_era && !claimed.is_zero() {
				available_unbonded += claimed;
			}
		}

		available_unbonded
	}

	pub fn withdraw_unbonded(who: &T::AccountId) -> sp_std::result::Result<Balance, DispatchError> {
		let current_era = Self::current_era();
		let claimed_unbond = <ClaimedUnbond<T>>::iter(who).collect::<Vec<(EraIndex, Balance)>>();
		let staking_currency_id = T::StakingCurrencyId::get();
		let mut withdrawn_amount: Balance = Zero::zero();

		for (era_index, claimed) in claimed_unbond {
			if era_index <= current_era && !claimed.is_zero() {
				if T::Currency::transfer(staking_currency_id, &Self::account_id(), who, claimed).is_ok() {
					withdrawn_amount += claimed;
					TotalClaimedUnbonded::mutate(|balance| *balance -= claimed);
					<ClaimedUnbond<T>>::remove(who, era_index);
				}
			}
		}
		Ok(withdrawn_amount)
	}

	pub fn bond(who: &T::AccountId, amount: Balance) -> sp_std::result::Result<Balance, DispatchError> {
		let liquid_exchange_rate = Self::liquid_exchange_rate();

		// bond dot
		T::Currency::ensure_can_withdraw(T::StakingCurrencyId::get(), who, amount)
			.map_err(|_| Error::<T>::StakingCurrencyNotEnough)?;
		T::Bridge::transfer_to_bridge(who, amount)?;
		T::Bridge::bond_extra(amount)?;
		TotalBonded::mutate(|bonded| *bonded += amount);

		// issue ldot to who
		let ldot_amount = ExchangeRate::from_natural(1)
			.checked_div(&liquid_exchange_rate)
			.unwrap_or_default()
			.saturating_mul_int(&amount);
		T::Currency::deposit(T::LiquidCurrencyId::get(), who, ldot_amount)?;

		<Module<T>>::deposit_event(RawEvent::BondAndMint(who.clone(), amount, ldot_amount));
		Ok(ldot_amount)
	}

	pub fn redeem_by_unbond(who: &T::AccountId, amount: Balance) -> DispatchResult {
		let mut ldot_to_redeem = amount;
		let liquid_exchange_rate = Self::liquid_exchange_rate();
		let mut unbond_amount = liquid_exchange_rate.saturating_mul_int(&ldot_to_redeem);
		let communal_bonded = Self::get_communal_bonded();

		if !unbond_amount.is_zero() && !communal_bonded.is_zero() {
			if unbond_amount > communal_bonded {
				// communal_bonded is not enough, calculate actual redeem ldot
				let new_redeem_amount: u128 = ExchangeRate::from_natural(1)
					.checked_div(&liquid_exchange_rate)
					.unwrap_or_default()
					.saturating_mul_int(&communal_bonded)
					.unique_saturated_into();
				ldot_to_redeem = new_redeem_amount.unique_saturated_into();
				unbond_amount = communal_bonded;
			}

			// burn who's ldot
			let liquid_currency_id = T::LiquidCurrencyId::get();
			T::Currency::ensure_can_withdraw(liquid_currency_id, who, ldot_to_redeem)
				.map_err(|_| Error::<T>::LiquidCurrencyNotEnough)?;
			T::Currency::withdraw(liquid_currency_id, who, ldot_to_redeem).expect("never failed after balance check");

			// start unbond at next era, and the unbond become unbonded after bonding duration
			let unbonded_era_index = Self::current_era()
				+ EraIndex::one()
				+ <<T as Trait>::Bridge as PolkadotBridgeType<_, _>>::BondingDuration::get();
			NextEraUnbond::mutate(|(unbond, claimed)| {
				*unbond += unbond_amount;
				*claimed += unbond_amount;
			});
			TotalBonded::mutate(|bonded| *bonded -= unbond_amount);
			<ClaimedUnbond<T>>::mutate(who, unbonded_era_index, |balance| *balance += unbond_amount);
			<Module<T>>::deposit_event(RawEvent::RedeemByUnbond(who.clone(), ldot_to_redeem));
		}

		Ok(())
	}

	pub fn claim_period_percent(era: EraIndex) -> Ratio {
		Ratio::from_rational(
			era.saturating_sub(Self::current_era()),
			<<T as Trait>::Bridge as PolkadotBridgeType<_, _>>::BondingDuration::get() + EraIndex::one(),
		)
	}

	pub fn calculate_claim_fee(amount: Balance, era: EraIndex) -> Balance {
		Ratio::from_natural(1)
			.saturating_sub(Self::claim_period_percent(era))
			.saturating_mul(T::MaxClaimFee::get())
			.saturating_mul_int(&amount)
	}

	pub fn redeem_by_free_unbonded(who: &T::AccountId, amount: Balance) -> DispatchResult {
		let current_era = Self::current_era();
		let mut total_deduct = amount;
		let mut fee = Self::calculate_claim_fee(total_deduct, current_era);
		let mut ldot_to_redeem = total_deduct.saturating_sub(fee);
		let liquid_exchange_rate = Self::liquid_exchange_rate();
		let mut unbond_amount = liquid_exchange_rate.saturating_mul_int(&ldot_to_redeem);
		let free_unbonded = Self::free_unbonded();

		if !unbond_amount.is_zero() && !free_unbonded.is_zero() {
			if unbond_amount > free_unbonded {
				// free_unbonded is not enough, re-calculate actual redeem ldot
				let new_ldot_to_redeem = ExchangeRate::from_natural(1)
					.checked_div(&liquid_exchange_rate)
					.unwrap_or_default()
					.saturating_mul_int(&free_unbonded);

				// re-assign
				fee = Ratio::from_rational(new_ldot_to_redeem, ldot_to_redeem).saturating_mul_int(&fee);
				ldot_to_redeem = new_ldot_to_redeem;
				unbond_amount = free_unbonded;
				total_deduct = fee + ldot_to_redeem;
			}

			let liquid_currency_id = T::LiquidCurrencyId::get();
			let staking_currency_id = T::StakingCurrencyId::get();
			T::Currency::ensure_can_withdraw(liquid_currency_id, who, total_deduct)
				.map_err(|_| Error::<T>::LiquidCurrencyNotEnough)?;
			T::Currency::transfer(staking_currency_id, &Self::account_id(), who, unbond_amount)?;
			FreeUnbonded::mutate(|balance| *balance -= unbond_amount);
			T::Currency::withdraw(liquid_currency_id, who, total_deduct).expect("never failed after balance check");

			let commission_fee = Ratio::from_natural(1)
				.saturating_sub(T::ClaimFeeReturnRatio::get())
				.saturating_mul_int(&fee);
			T::OnCommission::on_commission(liquid_currency_id, commission_fee);

			<Module<T>>::deposit_event(RawEvent::RedeemByFreeUnbonded(
				who.clone(),
				fee,
				ldot_to_redeem,
				unbond_amount,
			));
		}

		Ok(())
	}

	pub fn redeem_by_claim_unbonding(who: &T::AccountId, amount: Balance, target_era: EraIndex) -> DispatchResult {
		let current_era = Self::current_era();
		let bonding_duration = <<T as Trait>::Bridge as PolkadotBridgeType<_, _>>::BondingDuration::get();
		ensure!(
			target_era > current_era && target_era <= current_era + bonding_duration,
			Error::<T>::InvalidEra,
		);

		let mut total_deduct = amount;
		let mut fee = Self::calculate_claim_fee(total_deduct, target_era);
		let mut ldot_to_redeem = total_deduct.saturating_sub(fee);
		let liquid_exchange_rate = Self::liquid_exchange_rate();
		let mut unbond_amount = liquid_exchange_rate.saturating_mul_int(&ldot_to_redeem);
		let target_era_unbonding = Self::unbonding(target_era);
		let target_era_unclaimed = target_era_unbonding.0.saturating_sub(target_era_unbonding.1);

		if !unbond_amount.is_zero() && !target_era_unclaimed.is_zero() {
			if unbond_amount > target_era_unclaimed {
				// target_era_unclaimed is not enough, re-calculate actual redeem ldot
				let new_ldot_to_redeem = ExchangeRate::from_natural(1)
					.checked_div(&liquid_exchange_rate)
					.unwrap_or_default()
					.saturating_mul_int(&target_era_unclaimed);

				// re-assign
				fee = Ratio::from_rational(new_ldot_to_redeem, ldot_to_redeem).saturating_mul_int(&fee);
				ldot_to_redeem = new_ldot_to_redeem;
				unbond_amount = target_era_unclaimed;
				total_deduct = fee + ldot_to_redeem;
			}

			let liquid_currency_id = T::LiquidCurrencyId::get();
			T::Currency::ensure_can_withdraw(liquid_currency_id, who, total_deduct)
				.map_err(|_| Error::<T>::LiquidCurrencyNotEnough)?;
			T::Currency::withdraw(liquid_currency_id, who, total_deduct).expect("never failed after balance check");

			<ClaimedUnbond<T>>::mutate(who, target_era, |balance| *balance += unbond_amount);
			Unbonding::mutate(target_era, |(_, claimed)| *claimed += unbond_amount);
			UnbondingToFree::mutate(|balance| *balance = balance.saturating_sub(unbond_amount));

			let commission_fee = Ratio::from_natural(1)
				.saturating_sub(T::ClaimFeeReturnRatio::get())
				.saturating_mul_int(&fee);
			T::OnCommission::on_commission(liquid_currency_id, commission_fee);

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
		let (total_to_unbond, claimed_to_unbond) = Self::next_era_unbond();
		let bonding_duration = <<T as Trait>::Bridge as PolkadotBridgeType<_, _>>::BondingDuration::get();
		let unbonded_era_index = era + bonding_duration;

		if !total_to_unbond.is_zero() {
			if T::Bridge::unbond(total_to_unbond).is_ok() {
				NextEraUnbond::kill();
				TotalBonded::mutate(|bonded| *bonded -= total_to_unbond);
				Unbonding::insert(unbonded_era_index, (total_to_unbond, claimed_to_unbond));
				UnbondingToFree::mutate(|unbonding| *unbonding += total_to_unbond - claimed_to_unbond);
			}
		}
	}

	pub fn rebalance(era: EraIndex) {
		// #1: bridge withdraw unbonded and withdraw payout
		T::Bridge::withdraw_unbonded();

		// TODO: record the balances of bridge before and after do payout_nominator,
		// and oncommision to homa treasury according to RewardFeeRatio
		T::Bridge::payout_nominator();

		// #2: update staking pool by bridge ledger
		// TODO: adjust the amount of this era unbond by the slash situation in last era
		let bridge_ledger = T::Bridge::ledger();
		TotalBonded::put(bridge_ledger.active);

		// #3: withdraw available from bridge ledger and update unbonded at this era
		let bridge_available = T::Bridge::balance().saturating_sub(bridge_ledger.total);
		if T::Bridge::receive_from_bridge(&Self::account_id(), bridge_available).is_ok() {
			let (total_unbonded, claimed_unbonded) = Self::unbonding(era);
			let claimed_unbonded_added = bridge_available.min(claimed_unbonded);
			let free_unbonded_added = bridge_available.saturating_sub(claimed_unbonded_added);
			if !claimed_unbonded_added.is_zero() {
				TotalClaimedUnbonded::mutate(|balance| *balance += claimed_unbonded_added);
			}
			if !free_unbonded_added.is_zero() {
				FreeUnbonded::mutate(|balance| *balance += free_unbonded_added);
			}
			UnbondingToFree::mutate(|balance| *balance = balance.saturating_sub(total_unbonded - claimed_unbonded));
			Unbonding::remove(era);
		}

		// #4: according to the communal_bonded_ratio, decide to
		// bond extra amount to bridge or unbond system bonded to free pool at this era
		let communal_bonded_ratio = Self::get_communal_bonded_ratio();
		let max_bond_ratio = T::MaxBondRatio::get();
		let min_bond_ratio = T::MinBondRatio::get();
		let total_communal_balance = Self::get_total_communal_balance();
		if communal_bonded_ratio > max_bond_ratio {
			// unbond some to free pool
			let unbond_to_free = communal_bonded_ratio
				.saturating_sub(max_bond_ratio)
				.saturating_mul_int(&total_communal_balance)
				.min(Self::get_communal_bonded());

			if !unbond_to_free.is_zero() {
				NextEraUnbond::mutate(|(unbond, _)| *unbond += unbond_to_free);
			}
		} else if communal_bonded_ratio < min_bond_ratio {
			// bond more
			let bond_amount = min_bond_ratio
				.saturating_sub(communal_bonded_ratio)
				.saturating_mul_int(&total_communal_balance)
				.min(Self::free_unbonded());

			if T::Bridge::transfer_to_bridge(&Self::account_id(), bond_amount).is_ok() {
				FreeUnbonded::mutate(|balance| *balance -= bond_amount);
				if T::Bridge::bond_extra(bond_amount).is_ok() {
					TotalBonded::mutate(|bonded| *bonded += bond_amount);
				}
			}
		}

		// #5: unbond and update
		Self::unbond_and_update(era);
	}
}

impl<T: Trait> OnNewEra<EraIndex> for Module<T> {
	fn on_new_era(new_era: EraIndex) {
		CurrentEra::put(new_era);

		// rebalance first
		Self::rebalance(new_era);

		// nominate
		T::Bridge::nominate(T::Nominees::nominees());
	}
}

impl<T: Trait> HomaProtocol<T::AccountId, Balance, EraIndex> for Module<T> {
	type Balance = Balance;

	fn mint(who: &T::AccountId, amount: Self::Balance) -> sp_std::result::Result<Self::Balance, DispatchError> {
		Self::bond(who, amount)
	}

	fn redeem_by_unbond(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		Self::redeem_by_unbond(who, amount)
	}

	fn redeem_by_free_unbonded(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		Self::redeem_by_free_unbonded(who, amount)
	}

	fn redeem_by_claim_unbonding(who: &T::AccountId, amount: Self::Balance, target_era: EraIndex) -> DispatchResult {
		Self::redeem_by_claim_unbonding(who, amount, target_era)
	}

	fn withdraw_redemption(who: &T::AccountId) -> sp_std::result::Result<Self::Balance, DispatchError> {
		Self::withdraw_unbonded(who)
	}
}
