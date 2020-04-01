#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::decl_module;
use orml_traits::BasicCurrency;
use sp_runtime::RuntimeDebug;
use support::EraIndex;
use system::{self as system, ensure_signed};

#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq)]
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

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		pub fn mint(origin, amount: StakingBalanceOf<T>) {
			let who = ensure_signed(origin)?;
			<staking_pool::Module<T>>::bond(&who, amount)?;
		}

		pub fn redeem(origin, amount: LiquidBalanceOf<T>, strategy: RedeemStrategy) {
			let who = ensure_signed(origin)?;
			match strategy {
				RedeemStrategy::Immedately => {
					<staking_pool::Module<T>>::redeem_by_free_unbonded(&who, amount)?;
				},
				RedeemStrategy::Target(target_era) => {
					<staking_pool::Module<T>>::redeem_by_claim_unbonding(&who, amount, target_era)?;
				},
				RedeemStrategy::WaitForUnbonding => {
					<staking_pool::Module<T>>::redeem_by_unbond(&who, amount)?;
				},
			}
		}

		pub fn withdraw_redemption(origin) {
			let who = ensure_signed(origin)?;
			<staking_pool::Module<T>>::withdraw_unbonded(&who)?;
		}
	}
}

impl<T: Trait> Module<T> {}
