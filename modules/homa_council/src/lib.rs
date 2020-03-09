#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, ensure,
	traits::Get,
	traits::{Currency, LockableCurrency},
	Parameter,
};
use rstd::fmt::Debug;
use rstd::prelude::*;
use sp_runtime::{
	traits::{MaybeDisplay, MaybeSerializeDeserialize, Member},
	RuntimeDebug,
};
use support::{EraIndex, NomineesProvider};
use system::{self as system, ensure_signed};

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct UnlockChunk<Balance, EraIndex> {
	value: Balance,
	era: EraIndex,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, Default)]
pub struct BondingLedger<Balance, EraIndex> {
	pub total: Balance,
	pub active: Balance,
	pub unlocking: Vec<UnlockChunk<Balance, EraIndex>>,
}

type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
	type PolkadotAccountId: Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeDisplay + Ord + Default;
	type NominateesCount: Get<u32>;
	type MinBondThreshold: Get<BalanceOf<Self>>;
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
	/// Error for homa council module.
	pub enum Error for Module<T: Trait> {
		AuctionNotExsits,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as HomaCouncil {
		pub Nominations get(nominations): map hasher(twox_64_concat) T::AccountId => Vec<T::PolkadotAccountId>;
		pub Ledger get(ledger): map hasher(twox_64_concat) T::AccountId => BondingLedger<BalanceOf<T>, EraIndex>;
		pub Votes get(votes): map hasher(twox_64_concat) T::PolkadotAccountId => BalanceOf<T>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		const NominateesCount: u32 = T::NominateesCount::get();
		const MinBondThreshold: BalanceOf<T> = T::MinBondThreshold::get();

		pub fn bond(origin, amount: BalanceOf<T>) {

		}

		pub fn unbond(origin, amount: BalanceOf<T>) {

		}

		pub fn withdraw_unbond(origin) {

		}

		pub fn nominate(origin, targets: Vec<T::PolkadotAccountId>) {

		}

		pub fn chill(origin) {

		}
	}
}

impl<T: Trait> Module<T> {}

impl<T: Trait> NomineesProvider<T::AccountId> for Module<T> {
	fn nominees() -> Vec<T::AccountId> {
		vec![]
	}
}
