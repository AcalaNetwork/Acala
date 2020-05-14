#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::decl_module;
use frame_system::{self as system, ensure_signed};
use primitives::{Balance, EraIndex};
use sp_runtime::RuntimeDebug;
use support::HomaProtocol;

#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq)]
pub enum RedeemStrategy {
	Immediately,
	Target(EraIndex),
	WaitForUnbonding,
}

pub trait Trait: system::Trait {
	type Homa: HomaProtocol<Self::AccountId, Balance, EraIndex>;
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		#[weight = frame_support::weights::SimpleDispatchInfo::default()]
		pub fn mint(origin, #[compact] amount: Balance) {
			let who = ensure_signed(origin)?;
			T::Homa::mint(&who, amount)?;
		}

		#[weight = frame_support::weights::SimpleDispatchInfo::default()]
		pub fn redeem(origin, #[compact] amount: Balance, strategy: RedeemStrategy) {
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

		#[weight = frame_support::weights::SimpleDispatchInfo::default()]
		pub fn withdraw_redemption(origin) {
			let who = ensure_signed(origin)?;
			T::Homa::withdraw_redemption(&who)?;
		}
	}
}

impl<T: Trait> Module<T> {}
