#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::decl_module;
use frame_system::{self as system, ensure_signed};
use orml_utilities::with_transaction_result;
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
		#[weight = 10_000]
		pub fn mint(origin, #[compact] amount: Balance) {
			with_transaction_result(|| {
				let who = ensure_signed(origin)?;
				T::Homa::mint(&who, amount)?;
				Ok(())
			})?;
		}

		#[weight = 10_000]
		pub fn redeem(origin, #[compact] amount: Balance, strategy: RedeemStrategy) {
			with_transaction_result(|| {
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
				Ok(())
			})?;
		}

		#[weight = 10_000]
		pub fn withdraw_redemption(origin) {
			with_transaction_result(|| {
				let who = ensure_signed(origin)?;
				T::Homa::withdraw_redemption(&who)?;
				Ok(())
			})?;
		}
	}
}

impl<T: Trait> Module<T> {}
