#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, HasCompact};
use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get};
use orml_traits::BasicCurrency;
use rstd::prelude::*;
use sp_runtime::{
	traits::{AtLeast32Bit, CheckedSub, Saturating, Zero},
	RuntimeDebug,
};
use support::{
	EraIndex, NomineesProvider, OnCommission, OnNewEra, PolkadotBridge, PolkadotBridgeCall, PolkadotBridgeState,
	PolkadotBridgeType, Rate, Ratio,
};
use system::{self as system, ensure_root, ensure_signed};

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct UnlockChunk<Balance> {
	pub value: Balance,
	pub era: EraIndex,
	pub claimed: Balance,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, Default)]
pub struct StakingLedger<Balance> {
	pub total_bonded: Balance,
	pub active: Balance,
	pub unlocking: Vec<UnlockChunk<Balance>>,
	pub free: Balance,
}

impl<Balance> StakingLedger<Balance>
where
	Balance: HasCompact + Copy + Saturating + AtLeast32Bit,
{
	/// Remove entries from `unlocking` that are sufficiently old and reduce the
	/// total by the sum of their balances, and increase the free by the sum of
	/// their claimed.
	fn consolidate_unlocked(self, current_era: EraIndex) -> Self {
		let mut total_bonded = self.total_bonded;
		let mut free = self.free;
		let unlocking = self
			.unlocking
			.into_iter()
			.filter(|chunk| {
				if chunk.era > current_era {
					true
				} else {
					total_bonded = total_bonded.saturating_sub(chunk.value);
					free = free.saturating_add(chunk.claimed);
					false
				}
			})
			.collect();

		Self {
			total_bonded: total_bonded,
			active: self.active,
			unlocking: unlocking,
			free: free,
		}
	}

	/// Re-bond funds that were scheduled for unlocking.
	fn rebond(mut self, value: Balance) -> Self {
		let mut unlocking_balance: Balance = Zero::zero();

		while let Some(last) = self.unlocking.last_mut() {
			if unlocking_balance + last.value <= value {
				unlocking_balance += last.value;
				self.active += last.value;
				self.unlocking.pop();
			} else {
				let diff = value - unlocking_balance;

				unlocking_balance += diff;
				self.active += diff;
				last.value -= diff;
			}

			if unlocking_balance >= value {
				break;
			}
		}

		self
	}
}

type StakingBalanceOf<T> = <<T as Trait>::StakingCurrency as BasicCurrency<<T as system::Trait>::AccountId>>::Balance;
type LiquidBalanceOf<T> = <<T as Trait>::LiquidCurrency as BasicCurrency<<T as system::Trait>::AccountId>>::Balance;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type StakingCurrency: BasicCurrency<Self::AccountId>;
	type LiquidCurrency: BasicCurrency<Self::AccountId>;
	type Nominees: NomineesProvider<Self::AccountId>;
	type OnCommission: OnCommission<StakingBalanceOf<Self>>;
	type Bridge: PolkadotBridge<Self::BlockNumber, StakingBalanceOf<Self>>;
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
		pub ClaimedUnlockChunk get(claimed_unlock_chunk): double_map hasher(twox_64_concat) EraIndex, hasher(twox_64_concat) T::AccountId => StakingBalanceOf<T>;
		pub Ledger get(ledger): StakingLedger<StakingBalanceOf<T>>;
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
	pub fn rebalance() {
		let ledger = Self::ledger();
		let bonded_ratio = Ratio::from_rational(ledger.total_bonded, ledger.total_bonded.saturating_add(ledger.free));

		if bonded_ratio > T::MaxBondRatio::get() {
			// bond more
		} else if bonded_ratio < T::MinBondRatio::get() {
			// unbond some
		}
	}

	pub fn bond(amount: StakingBalanceOf<T>) {}

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
	fn on_new_era(era: EraIndex) {}
}
