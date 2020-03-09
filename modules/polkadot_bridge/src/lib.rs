#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get, Parameter};
use orml_traits::BasicCurrency;
use rstd::fmt::Debug;
use rstd::prelude::*;
use sp_runtime::{
	traits::{MaybeDisplay, MaybeSerializeDeserialize, Member},
	Permill,
};
use support::{
	EraIndex, OnNewEra, PolkadotBridge, PolkadotBridgeCall, PolkadotBridgeState, PolkadotBridgeType, Ratio,
	StakingLedger,
};
use system::{self as system, ensure_signed};

type BalanceOf<T> = <<T as Trait>::DotCurrency as BasicCurrency<<T as system::Trait>::AccountId>>::Balance;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type DotCurrency: BasicCurrency<Self::AccountId>;
	type OnNewEra: OnNewEra<EraIndex>;
	type MockRewardPercent: Get<Permill>;
	type BondingDuration: Get<EraIndex>;
	type EraLength: Get<Self::BlockNumber>;
	type PolkadotAccountId: Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeDisplay + Ord + Default;
}

decl_event!(
	pub enum Event<T>
	where
		<T as system::Trait>::AccountId,
		Balance = BalanceOf<T>,
	{
		Mint(AccountId, Balance),
	}
);

decl_error! {
	/// Error for polkadot bridge module.
	pub enum Error for Module<T: Trait> {
		AuctionNotExsits,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as PolkadotBridge {
		pub Bonded get(bonded): BalanceOf<T>;
		pub Available get(available): BalanceOf<T>;
		pub Unbonding get(unbonding): Vec<(BalanceOf<T>, T::BlockNumber)>;
		pub CurrentEra get(current_era): EraIndex;
		pub ForcedEra get(forced_era): Option<T::BlockNumber>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		const MockRewardPercent: Permill = T::MockRewardPercent::get();
		const BondingDuration: EraIndex = T::BondingDuration::get();
		const EraLength: T::BlockNumber = T::EraLength::get();

		pub fn simulate_slash(origin, amount: BalanceOf<T>) {

		}

		pub fn simualte_receive(origin, amount: BalanceOf<T>) {
		}

		pub fn simulate_redeem(origin, to: T::PolkadotAccountId, amount: BalanceOf<T>) {

		}

		pub fn force_era(origin, at: T::BlockNumber) {

		}
	}
}

impl<T: Trait> Module<T> {}

impl<T: Trait> PolkadotBridgeType<EraIndex, T::BlockNumber> for Module<T> {
	type BondingDuration = T::BondingDuration;
	type EraLength = T::EraLength;
	type PolkadotAccountId = T::PolkadotAccountId;
}

impl<T: Trait> PolkadotBridgeCall<EraIndex, T::BlockNumber, BalanceOf<T>> for Module<T> {
	fn bond_extra(amount: BalanceOf<T>) {}

	fn unbond(amount: BalanceOf<T>) {}

	fn rebond(amount: BalanceOf<T>) {}

	fn withdraw_unbonded() {}

	fn nominate(targets: Vec<Self::PolkadotAccountId>) {}

	fn transfer(to: Self::PolkadotAccountId, amount: BalanceOf<T>) {}

	fn payout_nominator() {}
}

impl<T: Trait> PolkadotBridgeState<BalanceOf<T>, EraIndex> for Module<T> {
	fn ledger() -> StakingLedger<BalanceOf<T>, EraIndex> {
		Default::default()
	}
	fn balance() -> BalanceOf<T> {
		Default::default()
	}

	fn current_era() -> EraIndex {
		Default::default()
	}
}

impl<T: Trait> PolkadotBridge<EraIndex, T::BlockNumber, BalanceOf<T>> for Module<T> {}
