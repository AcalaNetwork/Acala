#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get};
use orml_traits::BasicCurrency;
use rstd::prelude::*;
use sp_runtime::{Permill, RuntimeDebug};
use support::{EraIndex, NomineesProvider, OnNewEra, PolkadotBridge, Ratio, StakingLedger};
use system::{self as system, ensure_signed};

pub trait OnCommission<Balance> {
	fn on_commission(amount: Balance);
}

type StakingBalanceOf<T> = <<T as Trait>::StakingCurrency as BasicCurrency<<T as system::Trait>::AccountId>>::Balance;
type LiquidBalanceOf<T> = <<T as Trait>::LiquidCurrency as BasicCurrency<<T as system::Trait>::AccountId>>::Balance;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type StakingCurrency: BasicCurrency<Self::AccountId>;
	type LiquidCurrency: BasicCurrency<Self::AccountId>;
	type Nominees: NomineesProvider<Self::AccountId>;
	type OnCommission: OnCommission<StakingBalanceOf<Self>>;
	type Bridge: PolkadotBridge<EraIndex, Self::BlockNumber, StakingBalanceOf<Self>>;
	type MaxBondPercent: Get<Permill>;
	type MinBondPercent: Get<Permill>;
	type MaxClaimFee: Get<Permill>;
	type Commission: Get<Permill>;
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
		pub Ledger get(ledger): StakingLedger<StakingBalanceOf<T>, EraIndex>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		const MaxBondPercent: Permill = T::MaxBondPercent::get();
		const MinBondPercent: Permill = T::MinBondPercent::get();
		const MaxClaimFee: Permill = T::MaxClaimFee::get();
		const Commission: Permill = T::Commission::get();

		pub fn claim_payout(origin, amount: StakingBalanceOf<T>, proof: Vec<u8>) {

		}
	}
}

impl<T: Trait> Module<T> {
	pub fn rebalance() {}

	pub fn bond(amount: StakingBalanceOf<T>) {}

	pub fn unbond(amount: StakingBalanceOf<T>) {}

	pub fn claim(amount: LiquidBalanceOf<T>, era: EraIndex) {}

	pub fn claim_amount_percent(amount: StakingBalanceOf<T>, era: EraIndex) -> Ratio {
		Default::default()
	}

	pub fn claim_period_percent(era: EraIndex) -> Ratio {
		Default::default()
	}

	pub fn claim_fee(amount: StakingBalanceOf<T>, era: EraIndex) {}
}

impl<T: Trait> OnNewEra<EraIndex> for Module<T> {
	fn on_new_era(era: EraIndex) {}
}
