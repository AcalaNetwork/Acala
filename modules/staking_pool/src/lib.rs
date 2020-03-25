#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, HasCompact};
use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get};
use orml_traits::BasicCurrency;
use rstd::prelude::*;
use sp_runtime::{
	traits::{AccountIdConversion, AtLeast32Bit, CheckedSub, Saturating, Zero},
	ModuleId, RuntimeDebug,
};
use support::{
	EraIndex, NomineesProvider, OnCommission, OnNewEra, PolkadotBridge, PolkadotBridgeCall, PolkadotBridgeState,
	PolkadotBridgeType, Rate, Ratio,
};
use system::{self as system, ensure_root, ensure_signed};

// fn consolidate_unlocked(self, current_era: EraIndex) -> Self {
// 	let mut total_bonded = self.total_bonded;
// 	let mut free = self.free;
// 	let unlocking = self
// 		.unlocking
// 		.into_iter()
// 		.filter(|chunk| {
// 			if chunk.era > current_era {
// 				true
// 			} else {
// 				total_bonded = total_bonded.saturating_sub(chunk.value);
// 				free = free.saturating_add(chunk.claimed);
// 				false
// 			}
// 		})
// 		.collect();

// 	Self {
// 		total_bonded: total_bonded,
// 		active: self.active,
// 		unlocking: unlocking,
// 		free: free,
// 	}
// }

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
	type OnCommission: OnCommission<StakingBalanceOf<Self>>;
	type Bridge: PolkadotBridge<Self::BlockNumber, StakingBalanceOf<Self>, Self::AccountId>;
	type MaxBondRatio: Get<Ratio>;
	type MinBondRatio: Get<Ratio>;
	type MaxClaimFee: Get<Rate>;
	type Commission: Get<Rate>;
}

decl_event!(
	pub enum Event<T>
	where
		<T as system::Trait>::AccountId,
		StakingBalance = StakingBalanceOf<T>,
		LiquidBalance = LiquidBalanceOf<T>,
	{
		Mint(AccountId, StakingBalance, LiquidBalance),
	}
);

decl_error! {
	/// Error for staking pool module.
	pub enum Error for Module<T: Trait> {
		AuctionNotExsits,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as StakingPool {
		pub NextEraUnbond get(new_era_unbond): (StakingBalanceOf<T>, StakingBalanceOf<T>);
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
		const Commission: Rate = T::Commission::get();

		pub fn claim_payout(origin, amount: StakingBalanceOf<T>, proof: Vec<u8>) {
			ensure_root(origin)?;
		}
	}
}

impl<T: Trait> Module<T> {
	pub fn account_id() -> T::AccountId {
		MODULE_ID.into_account()
	}

	// the total balance is all DOT that belong to all existing LDOT
	pub fn get_total_balance() -> StakingBalanceOf<T> {
		Self::total_bonded()
			.saturating_add(Self::free_pool())
			.saturating_add(Self::unbonding_to_free())
	}

	// bonded_ratio = total_bonded / total_balance
	pub fn get_bonded_ratio() -> Ratio {
		Ratio::from_rational(Self::total_bonded(), Self::get_total_balance())
	}

	pub fn rebalance(era: EraIndex) {
		// #1: bridge withdraw unbonded and withdraw payout
		T::Bridge::withdraw_unbonded();
		T::Bridge::payout_nominator();

		// #2: update staking pool by bridger ledger
		let bridger_ledger = T::Bridge::ledger();
		<TotalBonded<T>>::put(bridger_ledger.active);

		// #3: withdraw available from bridger ledger
		let bridger_available = T::Bridge::balance().saturating_sub(bridger_ledger.total);
		T::Bridge::receive_from_bridge(&Self::account_id(), bridger_available);

		// #4: update unbonded
		let (total_unbonded, claimed_unbonded) = Self::unbonding(era);
		let claimed_unbonded_added = bridger_available.min(claimed_unbonded);
		let free_pool_added = bridger_available.saturating_sub(claimed_unbonded_added);
		if !claimed_unbonded_added.is_zero() {
			<TotalClaimedUnbonded<T>>::mutate(|balance| *balance += claimed_unbonded_added);
		}
		if !free_pool_added.is_zero() {
			<FreePool<T>>::mutate(|balance| *balance += free_pool_added);
		}
		<UnbondingToFree<T>>::mutate(|balance| *balance = balance.saturating_sub(total_unbonded - claimed_unbonded));
		<Unbonding<T>>::remove(era);

		// TODO: adjust the amount user unbond at this era by the slash amount in last era
		let (mut total_to_unbond, claimed_to_unbond) = <NextEraUnbond<T>>::take();

		// #5: according to bonded_ratio, decide to
		// bond extra amount to bridge or unbond system bonded to free pool at this era
		let bonded_ratio = Self::get_bonded_ratio();
		let max_bond_ratio = T::MaxBondRatio::get();
		let min_bond_ratio = T::MinBondRatio::get();
		let total_balance = Self::get_total_balance();
		if bonded_ratio > max_bond_ratio {
			// unbond some
			let extra_unbond_amount = bonded_ratio
				.saturating_sub(max_bond_ratio)
				.saturating_mul_int(&total_balance)
				.min(Self::total_bonded());

			if extra_unbond_amount.is_zero() {
				total_to_unbond += extra_unbond_amount;
				<UnbondingToFree<T>>::mutate(|unbonding| *unbonding += extra_unbond_amount);
			}
		} else if bonded_ratio < min_bond_ratio {
			// bond more
			let bond_amount = min_bond_ratio
				.saturating_sub(bonded_ratio)
				.saturating_mul_int(&total_balance)
				.min(Self::free_pool());

			T::Bridge::transfer_to_bridge(&Self::account_id(), bond_amount);
			T::Bridge::bond_extra(bond_amount);
		}

		// #6: unbond and update
		let bonding_duration =
			<<T as Trait>::Bridge as PolkadotBridgeType<<T as system::Trait>::BlockNumber>>::BondingDuration::get();
		let unbonded_era_index = era + bonding_duration;
		T::Bridge::unbond(total_to_unbond);
		<Unbonding<T>>::insert(unbonded_era_index, (total_to_unbond, claimed_to_unbond));
	}

	pub fn bond(who: &T::AccountId, amount: StakingBalanceOf<T>) {}

	pub fn unbond(amount: StakingBalanceOf<T>) {}

	pub fn claim(amount: LiquidBalanceOf<T>, era: EraIndex) {}

	pub fn claim_amount_percent(amount: StakingBalanceOf<T>, era: EraIndex) -> Ratio {
		let ledger = T::Bridge::ledger();
		let free = T::Bridge::balance().saturating_sub(ledger.total);
		let mut available: StakingBalanceOf<T> = Zero::zero();
		ledger.unlocking.into_iter().map(|x| {
			if x.era <= era {
				available = available.saturating_add(x.value);
			}
		});

		Ratio::from_rational(amount, free.saturating_add(available))
	}

	pub fn claim_period_percent(era: EraIndex) -> Ratio {
		Ratio::from_rational(
			era.checked_sub(T::Bridge::current_era()).unwrap_or_default(),
			<<T as Trait>::Bridge as PolkadotBridgeType<<T as system::Trait>::BlockNumber>>::BondingDuration::get(),
		)
	}

	pub fn claim_fee(amount: StakingBalanceOf<T>, era: EraIndex) {}
}

impl<T: Trait> OnNewEra for Module<T> {
	fn on_new_era(era: EraIndex) {
		// rebalance first
		Self::rebalance(era);

		// nominate
		T::Bridge::nominate(T::Nominees::nominees());
	}
}
