#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get, IterableStorageDoubleMap};
use frame_system::{self as system};
use orml_traits::MultiCurrency;
use primitives::{Balance, CurrencyId, EraIndex};
use sp_runtime::{
	traits::{AccountIdConversion, One, Saturating, Zero},
	DispatchError, DispatchResult, FixedPointNumber, ModuleId,
};
use sp_std::prelude::*;
use support::{
	ExchangeRate, HomaProtocol, NomineesProvider, OnCommission, OnNewEra, PolkadotBridge, PolkadotBridgeCall,
	PolkadotBridgeState, PolkadotBridgeType, Rate, Ratio,
};

mod mock;
mod tests;

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

	/// The staking pool's module id, keep all staking currency belong to Homa
	/// protocol.
	type ModuleId: Get<ModuleId>;

	// TODO: add RewardFeeRatio
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
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as StakingPool {
		pub CurrentEra get(fn current_era): EraIndex;

		pub NextEraUnbond get(fn next_era_unbond): (Balance, Balance);
		pub Unbonding get(fn unbonding): map hasher(twox_64_concat) EraIndex => (Balance, Balance); // (value, claimed), value - claimed = unbond to free

		pub ClaimedUnbond get(fn claimed_unbond): double_map hasher(twox_64_concat) T::AccountId, hasher(twox_64_concat) EraIndex => Balance;
		pub TotalClaimedUnbonded get(fn total_claimed_unbonded): Balance;

		pub TotalBonded get(fn total_bonded): Balance;
		pub UnbondingToFree get(fn unbonding_to_free): Balance;
		pub FreeUnbonded get(fn free_unbonded): Balance;
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
		const ModuleId: ModuleId = T::ModuleId::get();
	}
}

impl<T: Trait> Module<T> {
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

	/// communal_bonded_ratio = communal_bonded / total_communal_balance
	pub fn get_communal_bonded_ratio() -> Ratio {
		Ratio::checked_from_rational(Self::get_communal_bonded(), Self::get_total_communal_balance())
			.unwrap_or_default()
	}

	/// liquid currency / staking currency  = total communal staking currency /
	/// total supply of liquid currency
	pub fn liquid_exchange_rate() -> ExchangeRate {
		let total_communal_staking_amount = Self::get_total_communal_balance();
		let total_liquid_amount = T::Currency::total_issuance(T::LiquidCurrencyId::get());

		if !total_communal_staking_amount.is_zero() && !total_liquid_amount.is_zero() {
			ExchangeRate::checked_from_rational(total_communal_staking_amount, total_liquid_amount)
				.unwrap_or_else(T::DefaultExchangeRate::get)
		} else {
			T::DefaultExchangeRate::get()
		}
	}

	pub fn get_available_unbonded(who: &T::AccountId) -> Balance {
		let current_era = Self::current_era();
		<ClaimedUnbond<T>>::iter_prefix(who)
			.filter(|(era_index, _)| era_index <= &current_era)
			.fold(Zero::zero(), |available_unbonded, (_, claimed)| {
				available_unbonded.saturating_add(claimed)
			})
	}

	pub fn withdraw_unbonded(who: &T::AccountId) -> sp_std::result::Result<Balance, DispatchError> {
		let current_era = Self::current_era();
		let staking_currency_id = T::StakingCurrencyId::get();
		let mut withdrawn_amount: Balance = Zero::zero();

		<ClaimedUnbond<T>>::iter_prefix(who)
			.filter(|(era_index, _)| era_index <= &current_era)
			.for_each(|(era_index, claimed)| {
				withdrawn_amount = withdrawn_amount.saturating_add(claimed);
				<ClaimedUnbond<T>>::remove(who, era_index);
			});

		T::Currency::transfer(staking_currency_id, &Self::account_id(), who, withdrawn_amount)?;
		TotalClaimedUnbonded::mutate(|balance| *balance = balance.saturating_sub(withdrawn_amount));
		Ok(withdrawn_amount)
	}

	pub fn bond(amount: Balance) -> DispatchResult {
		if amount.is_zero() {
			return Ok(());
		}

		T::Bridge::transfer_to_bridge(&Self::account_id(), amount)?;
		T::Bridge::bond_extra(amount)?;
		FreeUnbonded::mutate(|free_unbonded| -> DispatchResult {
			*free_unbonded = free_unbonded.checked_sub(amount).ok_or(Error::<T>::Overflow)?;
			Ok(())
		})?;
		TotalBonded::try_mutate(|total_bonded| -> DispatchResult {
			*total_bonded = total_bonded.checked_add(amount).ok_or(Error::<T>::Overflow)?;
			Ok(())
		})
	}

	pub fn deposit_free_pool(who: &T::AccountId, amount: Balance) -> DispatchResult {
		if amount.is_zero() {
			return Ok(());
		}
		T::Currency::transfer(T::StakingCurrencyId::get(), who, &Self::account_id(), amount)?;
		FreeUnbonded::try_mutate(|free| -> DispatchResult {
			*free = free.checked_add(amount).ok_or(Error::<T>::Overflow)?;
			Ok(())
		})
	}

	pub fn claim_period_percent(era: EraIndex) -> Ratio {
		Ratio::checked_from_rational(
			era.saturating_sub(Self::current_era()),
			<<T as Trait>::Bridge as PolkadotBridgeType<_, _>>::BondingDuration::get().saturating_add(EraIndex::one()),
		)
		.unwrap_or_default()
	}

	pub fn calculate_claim_fee(amount: Balance, era: EraIndex) -> Balance {
		Ratio::one()
			.saturating_sub(Self::claim_period_percent(era))
			.saturating_mul(T::MaxClaimFee::get())
			.saturating_mul_int(amount)
	}

	/// This function must to be called in `with_transaction_result` scope to
	/// ensure atomic
	pub fn redeem_by_unbond(who: &T::AccountId, amount: Balance) -> DispatchResult {
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
			NextEraUnbond::try_mutate(|(unbond, claimed)| -> DispatchResult {
				*unbond = unbond
					.checked_add(staking_amount_to_unbond)
					.ok_or(Error::<T>::Overflow)?;
				*claimed = claimed
					.checked_add(staking_amount_to_unbond)
					.ok_or(Error::<T>::Overflow)?;
				Ok(())
			})?;
			<ClaimedUnbond<T>>::try_mutate(who, unbonded_era_index, |balance| -> DispatchResult {
				*balance = balance
					.checked_add(staking_amount_to_unbond)
					.ok_or(Error::<T>::Overflow)?;
				Ok(())
			})?;

			<Module<T>>::deposit_event(RawEvent::RedeemByUnbond(
				who.clone(),
				liquid_amount_to_redeem,
				staking_amount_to_unbond,
			));
		}

		Ok(())
	}

	/// This function must to be called in `with_transaction_result` scope to
	/// ensure atomic
	pub fn redeem_by_free_unbonded(who: &T::AccountId, amount: Balance) -> DispatchResult {
		let mut liquid_amount_to_redeem = amount;
		let mut fee_in_liquid_currency = sp_std::cmp::min(
			liquid_amount_to_redeem,
			Self::calculate_claim_fee(liquid_amount_to_redeem, Self::current_era()),
		);
		let mut liquid_amount_to_burn = liquid_amount_to_redeem.saturating_sub(fee_in_liquid_currency);
		let liquid_exchange_rate = Self::liquid_exchange_rate();
		let mut staking_amount = liquid_exchange_rate
			.checked_mul_int(liquid_amount_to_burn)
			.ok_or(Error::<T>::Overflow)?;
		let free_unbonded_pool = Self::free_unbonded();

		if !staking_amount.is_zero() && !free_unbonded_pool.is_zero() {
			// if free_unbonded_pool is not enough, need re-calculate
			if staking_amount > free_unbonded_pool {
				let ratio = Ratio::checked_from_rational(free_unbonded_pool, staking_amount)
					.expect("staking_amount is not zero; qed");
				liquid_amount_to_redeem = ratio.saturating_mul_int(liquid_amount_to_redeem);
				fee_in_liquid_currency = sp_std::cmp::min(
					liquid_amount_to_redeem,
					ratio.saturating_mul_int(fee_in_liquid_currency),
				);
				liquid_amount_to_burn = liquid_amount_to_redeem.saturating_sub(fee_in_liquid_currency);
				staking_amount = free_unbonded_pool;
			}

			let liquid_currency_id = T::LiquidCurrencyId::get();
			T::Currency::withdraw(liquid_currency_id, who, liquid_amount_to_redeem)
				.map_err(|_| Error::<T>::LiquidCurrencyNotEnough)?;
			T::Currency::transfer(T::StakingCurrencyId::get(), &Self::account_id(), who, staking_amount)?;
			FreeUnbonded::try_mutate(|free_unbonded| -> DispatchResult {
				*free_unbonded = free_unbonded.checked_sub(staking_amount).ok_or(Error::<T>::Overflow)?;
				Ok(())
			})?;

			let commission_to_homa = Ratio::one()
				.saturating_sub(T::ClaimFeeReturnRatio::get())
				.saturating_mul_int(fee_in_liquid_currency);
			T::OnCommission::on_commission(liquid_currency_id, commission_to_homa);

			<Module<T>>::deposit_event(RawEvent::RedeemByFreeUnbonded(
				who.clone(),
				fee_in_liquid_currency,
				liquid_amount_to_burn,
				staking_amount,
			));
		}

		Ok(())
	}

	/// This function must to be called in `with_transaction_result` scope to
	/// ensure atomic
	pub fn redeem_by_claim_unbonding(who: &T::AccountId, amount: Balance, target_era: EraIndex) -> DispatchResult {
		let current_era = Self::current_era();
		let bonding_duration = <<T as Trait>::Bridge as PolkadotBridgeType<_, _>>::BondingDuration::get();
		ensure!(
			target_era > current_era && target_era <= current_era + bonding_duration,
			Error::<T>::InvalidEra,
		);

		let mut liquid_amount_to_redeem = amount;
		let mut fee_in_liquid_currency = sp_std::cmp::min(
			liquid_amount_to_redeem,
			Self::calculate_claim_fee(liquid_amount_to_redeem, target_era),
		);
		let mut liquid_amount_to_burn = liquid_amount_to_redeem.saturating_sub(fee_in_liquid_currency);
		let mut staking_amount_to_claim = Self::liquid_exchange_rate().saturating_mul_int(liquid_amount_to_burn);
		let (unbonding_of_target_era, claimed_unbonding_of_target_era) = Self::unbonding(target_era);
		let available_unclaimed_unbonding = unbonding_of_target_era.saturating_sub(claimed_unbonding_of_target_era);

		if !staking_amount_to_claim.is_zero() && !available_unclaimed_unbonding.is_zero() {
			// if available_unclaimed_unbonding is not enough, need re-calculate
			if staking_amount_to_claim > available_unclaimed_unbonding {
				let ratio = Ratio::checked_from_rational(available_unclaimed_unbonding, staking_amount_to_claim)
					.expect("staking_amount_to_claim is not zero; qed");
				liquid_amount_to_redeem = ratio.saturating_mul_int(liquid_amount_to_redeem);
				fee_in_liquid_currency = sp_std::cmp::min(
					liquid_amount_to_redeem,
					ratio.saturating_mul_int(fee_in_liquid_currency),
				);
				liquid_amount_to_burn = liquid_amount_to_redeem.saturating_sub(fee_in_liquid_currency);
				staking_amount_to_claim = available_unclaimed_unbonding;
			}

			let liquid_currency_id = T::LiquidCurrencyId::get();
			T::Currency::withdraw(liquid_currency_id, who, liquid_amount_to_redeem)
				.map_err(|_| Error::<T>::LiquidCurrencyNotEnough)?;
			<ClaimedUnbond<T>>::try_mutate(who, target_era, |claimed_unbond| -> DispatchResult {
				*claimed_unbond = claimed_unbond
					.checked_add(staking_amount_to_claim)
					.ok_or(Error::<T>::Overflow)?;
				Ok(())
			})?;
			Unbonding::try_mutate(target_era, |(_, claimed_unbonding)| -> DispatchResult {
				*claimed_unbonding = claimed_unbonding
					.checked_add(staking_amount_to_claim)
					.ok_or(Error::<T>::Overflow)?;
				Ok(())
			})?;
			UnbondingToFree::try_mutate(|unbonding_to_free| -> DispatchResult {
				*unbonding_to_free = unbonding_to_free
					.checked_sub(staking_amount_to_claim)
					.ok_or(Error::<T>::Overflow)?;
				Ok(())
			})?;

			let commission_to_homa = Ratio::one()
				.saturating_sub(T::ClaimFeeReturnRatio::get())
				.saturating_mul_int(fee_in_liquid_currency);
			T::OnCommission::on_commission(liquid_currency_id, commission_to_homa);

			<Module<T>>::deposit_event(RawEvent::RedeemByClaimUnbonding(
				who.clone(),
				target_era,
				fee_in_liquid_currency,
				liquid_amount_to_burn,
				staking_amount_to_claim,
			));
		}

		Ok(())
	}

	pub fn unbond_and_update(era: EraIndex) {
		let (total_to_unbond, claimed_to_unbond) = Self::next_era_unbond();
		let bonding_duration = <<T as Trait>::Bridge as PolkadotBridgeType<_, _>>::BondingDuration::get();
		let unbonded_era_index = era.saturating_add(bonding_duration);

		if !total_to_unbond.is_zero() && T::Bridge::unbond(total_to_unbond).is_ok() {
			NextEraUnbond::kill();
			TotalBonded::mutate(|bonded| *bonded = bonded.saturating_sub(total_to_unbond));
			Unbonding::insert(unbonded_era_index, (total_to_unbond, claimed_to_unbond));
			UnbondingToFree::mutate(|unbonding| {
				*unbonding = unbonding.saturating_add(total_to_unbond.saturating_sub(claimed_to_unbond))
			});
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
				.saturating_mul_int(total_communal_balance)
				.min(Self::get_communal_bonded());

			if !unbond_to_free.is_zero() {
				NextEraUnbond::mutate(|(unbond, _)| *unbond = unbond.saturating_add(unbond_to_free));
			}
		} else if communal_bonded_ratio < min_bond_ratio {
			// bond more
			let bond_amount = min_bond_ratio
				.saturating_sub(communal_bonded_ratio)
				.saturating_mul_int(total_communal_balance)
				.min(Self::free_unbonded());

			// bound more amount for staking. if it failed, just that added amount did not
			// succeed and it should not affect the process. so ignore result to continue.
			let _ = Self::bond(bond_amount);
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

	/// This function must to be called in `with_transaction_result` scope to
	/// ensure atomic
	fn mint(who: &T::AccountId, amount: Self::Balance) -> sp_std::result::Result<Self::Balance, DispatchError> {
		Self::deposit_free_pool(who, amount)?;

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
