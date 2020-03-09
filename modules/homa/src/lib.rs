#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure};
use orml_traits::BasicCurrency;
use sp_runtime::RuntimeDebug;
use support::EraIndex;
use system::{self as system, ensure_signed};

#[cfg_attr(feature = "std", derive(PartialEq, Eq))]
#[derive(Encode, Decode, Clone, RuntimeDebug)]
pub enum RedeemStrategy {
	Immedately,
	Target(EraIndex),
	WaitForUnbonding,
}

type StakingBalanceOf<T> =
	<<T as staking_pool::Trait>::StakingCurrency as BasicCurrency<<T as system::Trait>::AccountId>>::Balance;
type LiquidBalanceOf<T> =
	<<T as staking_pool::Trait>::LiquidCurrency as BasicCurrency<<T as system::Trait>::AccountId>>::Balance;

pub trait Trait: system::Trait + staking_pool::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
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
	/// Error for homa module.
	pub enum Error for Module<T: Trait> {
		AuctionNotExsits,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Homa {}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		pub fn mint(origin, amount: StakingBalanceOf<T>) {

		}

		pub fn redeem(origin, amount: LiquidBalanceOf<T>, strategy: RedeemStrategy) {

		}
	}
}

impl<T: Trait> Module<T> {}
