#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{decl_error, decl_module, decl_storage};
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

pub trait Trait: system::Trait + staking_pool::Trait {}

decl_error! {
	/// Error for homa module.
	pub enum Error for Module<T: Trait> {
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Homa {}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		pub fn mint(origin, amount: StakingBalanceOf<T>) {
			let who = ensure_signed(origin)?;
			<staking_pool::Module<T>>::bond(&who, amount)?;
		}

		pub fn redeem(origin, amount: LiquidBalanceOf<T>, strategy: RedeemStrategy) {
			let who = ensure_signed(origin)?;
			match strategy {
				RedeemStrategy::Immedately => {
					<staking_pool::Module<T>>::redeem_by_free_pool(&who, amount)?;
				},
				RedeemStrategy::Target(target_era) => {
					<staking_pool::Module<T>>::redeem_by_claim_unbonding(&who, amount, target_era)?;
				},
				RedeemStrategy::WaitForUnbonding => {
					<staking_pool::Module<T>>::redeem_by_unbond(&who, amount)?;
				},
			}
		}
	}
}

impl<T: Trait> Module<T> {}
