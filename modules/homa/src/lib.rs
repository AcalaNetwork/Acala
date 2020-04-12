#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::decl_module;
use sp_runtime::RuntimeDebug;
use support::{EraIndex, HomaProtocol};
use system::{self as system, ensure_signed};

#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq)]
pub enum RedeemStrategy {
	Immediately,
	Target(EraIndex),
	WaitForUnbonding,
}

type BalanceOf<T> = <<T as Trait>::Homa as HomaProtocol<<T as system::Trait>::AccountId>>::Balance;

pub trait Trait: system::Trait {
	type Homa: HomaProtocol<Self::AccountId>;
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		pub fn mint(origin, #[compact] amount: BalanceOf<T>) {
			let who = ensure_signed(origin)?;
			T::Homa::mint(&who, amount)?;
		}

		pub fn redeem(origin, #[compact] amount: BalanceOf<T>, strategy: RedeemStrategy) {
			let who = ensure_signed(origin)?;
			match strategy {
				RedeemStrategy::Immediately => {
					T::Homa::redeem_by_free_unbonded(&who, amount)?;
				},
				RedeemStrategy::Target(target_era) => {
					T::Homa::redeem_by_claim_unbonding(&who, amount, target_era)?;
				},
				RedeemStrategy::WaitForUnbonding => {
					T::Homa::redeem_by_unbond(&who, amount)?;
				},
			}
		}

		pub fn withdraw_redemption(origin) {
			let who = ensure_signed(origin)?;
			T::Homa::withdraw_redemption(&who)?;
		}
	}
}

impl<T: Trait> Module<T> {}
