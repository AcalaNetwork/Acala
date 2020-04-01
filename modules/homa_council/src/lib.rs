#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
	decl_error, decl_module, decl_storage, ensure,
	traits::{Get, LockIdentifier},
	Parameter,
};
use orml_traits::{BasicCurrency, BasicLockableCurrency};
use rstd::{fmt::Debug, prelude::*};
use sp_runtime::{
	traits::{AtLeast32Bit, CheckedSub, MaybeDisplay, MaybeSerializeDeserialize, Member, Saturating, Zero},
	RuntimeDebug,
};
use support::{EraIndex, NomineesProvider, OnNewEra};
use system::{self as system, ensure_signed};

mod mock;
mod tests;

const HOMA_COUNCIL_ID: LockIdentifier = *b"homacncl";

/// Just a Balance/BlockNumber tuple to encode when a chunk of funds will be unlocked.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct UnlockChunk<Balance> {
	/// Amount of funds to be unlocked.
	value: Balance,
	/// Era number at which point it'll be unlocked.
	era: EraIndex,
}

/// The ledger of a (bonded) account.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, Default)]
pub struct BondingLedger<Balance> {
	/// The total amount of the account's balance that we are currently accounting for.
	/// It's just `active` plus all the `unlocking` balances.
	pub total: Balance,
	/// The total amount of the account's balance that will be at stake in any forthcoming rounds.
	pub active: Balance,
	/// Any balance that is becoming free, which may eventually be transferred out of the account.
	pub unlocking: Vec<UnlockChunk<Balance>>,
}

impl<Balance> BondingLedger<Balance>
where
	Balance: Copy + Saturating + AtLeast32Bit,
{
	/// Remove entries from `unlocking` that are sufficiently old and reduce the
	/// total by the sum of their balances.
	fn consolidate_unlocked(self, current_era: EraIndex) -> Self {
		let mut total = self.total;
		let unlocking = self
			.unlocking
			.into_iter()
			.filter(|chunk| {
				if chunk.era > current_era {
					true
				} else {
					total = total.saturating_sub(chunk.value);
					false
				}
			})
			.collect();

		Self {
			total: total,
			active: self.active,
			unlocking: unlocking,
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

type BalanceOf<T> = <<T as Trait>::Currency as BasicCurrency<<T as system::Trait>::AccountId>>::Balance;

pub trait Trait: system::Trait {
	type Currency: BasicLockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
	type PolkadotAccountId: Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeDisplay + Ord + Default;
	type MinBondThreshold: Get<BalanceOf<Self>>;
	type BondingDuration: Get<EraIndex>;
	type NominateesCount: Get<usize>;
	type MaxUnlockingChunks: Get<usize>;
}

decl_error! {
	/// Error for homa council module.
	pub enum Error for Module<T: Trait> {
		BelowMinBondThreshold,
		InvalidTargetsLength,
		TooManyChunks,
		NoBonded,
		NoUnlockChunk,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as HomaCouncil {
		pub Nominations get(nominations): map hasher(twox_64_concat) T::AccountId => Vec<T::PolkadotAccountId>;
		pub Ledger get(ledger): map hasher(twox_64_concat) T::AccountId => BondingLedger<BalanceOf<T>>;
		pub Votes get(votes): linked_map hasher(twox_64_concat) T::PolkadotAccountId => BalanceOf<T>;
		pub Nominees get(nominees): Vec<T::PolkadotAccountId>;
		pub CurrentEra get(current_era): EraIndex;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		const MinBondThreshold: BalanceOf<T> = T::MinBondThreshold::get();
		const NominateesCount: u32 = T::NominateesCount::get() as u32;
		const MaxUnlockingChunks: u32 = T::MaxUnlockingChunks::get() as u32;

		pub fn bond(origin, #[compact] amount: BalanceOf<T>) {
			let who = ensure_signed(origin)?;

			let mut ledger = Self::ledger(&who);
			let free_balance = T::Currency::free_balance(&who);
			if let Some(extra) = free_balance.checked_sub(&ledger.total) {
				let extra = extra.min(amount);
				let old_active = ledger.active;
				ledger.active += extra;
				ensure!(ledger.active >= T::MinBondThreshold::get(), Error::<T>::BelowMinBondThreshold);
				ledger.total += extra;
				let old_nominations = Self::nominations(&who);

				Self::update_votes(old_active, &old_nominations, ledger.active, &old_nominations);
				Self::update_ledger(&who, &ledger);
			}
		}

		pub fn unbond(origin, amount: BalanceOf<T>) {
			let who = ensure_signed(origin)?;

			let mut ledger = Self::ledger(&who);
			ensure!(
				ledger.unlocking.len() < T::MaxUnlockingChunks::get(),
				Error::<T>::TooManyChunks,
			);

			let amount = amount.min(ledger.active);

			if !amount.is_zero() {
				let old_active = ledger.active;
				ledger.active -= amount;

				ensure!(
					ledger.active.is_zero() || ledger.active >= T::MinBondThreshold::get(),
					Error::<T>::BelowMinBondThreshold,
				);

				// Note: in case there is no current era it is fine to bond one era more.
				let era = Self::current_era() + T::BondingDuration::get();
				ledger.unlocking.push(UnlockChunk{
					value: amount,
					era: era,
				});
				let old_nominations = Self::nominations(&who);

				Self::update_votes(old_active, &old_nominations, ledger.active, &old_nominations);
				Self::update_ledger(&who, &ledger);
			}
		}

		pub fn rebond(origin, amount: BalanceOf<T>) {
			let who = ensure_signed(origin)?;
			let ledger = Self::ledger(&who);
			ensure!(
				!ledger.unlocking.is_empty(),
				Error::<T>::NoUnlockChunk,
			);
			let old_active = ledger.active;
			let old_nominations = Self::nominations(&who);
			let ledger = ledger.rebond(amount);

			Self::update_votes(old_active, &old_nominations, ledger.active, &old_nominations);
			Self::update_ledger(&who, &ledger);
		}

		pub fn withdraw_unbonded(origin) {
			let who = ensure_signed(origin)?;
			let ledger = Self::ledger(&who).consolidate_unlocked(Self::current_era());

			if ledger.unlocking.is_empty() && ledger.active.is_zero() {
				Self::remove_ledger(&who);
			} else {
				// This was the consequence of a partial unbond. just update the ledger and move on.
				Self::update_ledger(&who, &ledger);
			}
		}

		pub fn nominate(origin, targets: Vec<T::PolkadotAccountId>) {
			let who = ensure_signed(origin)?;
			ensure!(
				!targets.is_empty() &&
				targets.len() <= T::NominateesCount::get(),
				Error::<T>::InvalidTargetsLength,
			);

			let ledger = Self::ledger(&who);
			ensure!(!ledger.total.is_zero(), Error::<T>::NoBonded);

			let mut targets = targets.clone();
			targets.sort();
			targets.dedup();

			let old_nominations = Self::nominations(&who);
			let old_active = Self::ledger(&who).active;

			Self::update_votes(old_active, &old_nominations, old_active, &targets);
			<Nominations<T>>::insert(&who, &targets);
		}

		pub fn chill(origin) {
			let who = ensure_signed(origin)?;

			let old_nominations = Self::nominations(&who);
			let old_active = Self::ledger(&who).active;

			Self::update_votes(old_active, &old_nominations, Zero::zero(), &vec![]);
			<Nominations<T>>::remove(&who);
		}
	}
}

impl<T: Trait> Module<T> {
	fn update_ledger(who: &T::AccountId, ledger: &BondingLedger<BalanceOf<T>>) {
		T::Currency::set_lock(HOMA_COUNCIL_ID, who, ledger.total);
		<Ledger<T>>::insert(who, ledger);
	}

	fn remove_ledger(who: &T::AccountId) {
		T::Currency::remove_lock(HOMA_COUNCIL_ID, who);
		<Ledger<T>>::remove(who);
		<Nominations<T>>::remove(who);
	}

	fn update_votes(
		old_active: BalanceOf<T>,
		old_nominations: &Vec<T::PolkadotAccountId>,
		new_active: BalanceOf<T>,
		new_nominations: &Vec<T::PolkadotAccountId>,
	) {
		if !old_active.is_zero() && !old_nominations.is_empty() {
			for account in old_nominations {
				let votes = Self::votes(account).saturating_sub(old_active);
				if votes.is_zero() {
					<Votes<T>>::remove(account);
				} else {
					<Votes<T>>::insert(account, votes);
				}
			}
		}

		if !new_active.is_zero() && !new_nominations.is_empty() {
			for account in new_nominations {
				<Votes<T>>::mutate(account, |balance| *balance = balance.saturating_add(new_active));
			}
		}
	}

	fn rebalance() {
		let mut voters = <Votes<T>>::enumerate().collect::<Vec<(T::PolkadotAccountId, BalanceOf<T>)>>();

		voters.sort_by(|a, b| b.1.cmp(&a.1));

		let new_nominees = voters
			.into_iter()
			.take(T::NominateesCount::get())
			.map(|(nominee, _)| nominee)
			.collect::<Vec<_>>();

		<Nominees<T>>::put(new_nominees);
	}
}

impl<T: Trait> NomineesProvider<T::PolkadotAccountId> for Module<T> {
	fn nominees() -> Vec<T::PolkadotAccountId> {
		Self::rebalance(); // can remove the operation by ensure homa_council::on_new_era execute before staking_pool::on_new_era
		<Nominees<T>>::get()
	}
}

impl<T: Trait> OnNewEra for Module<T> {
	fn on_new_era(era: EraIndex) {
		CurrentEra::put(era);
		Self::rebalance();
	}
}
