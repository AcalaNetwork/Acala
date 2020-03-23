use codec::{Decode, Encode, HasCompact};
use frame_support::{traits::Get, Parameter};
use rstd::fmt::Debug;
use rstd::prelude::*;
use sp_runtime::{
	traits::{AtLeast32Bit, MaybeDisplay, MaybeSerializeDeserialize, Member, Saturating, Zero},
	RuntimeDebug,
};

/// Counter for the number of eras that have passed.
pub type EraIndex = u32;

#[impl_trait_for_tuples::impl_for_tuples(30)]
pub trait OnNewEra {
	fn on_new_era(era: EraIndex);
}

pub trait NomineesProvider<AccountId> {
	fn nominees() -> Vec<AccountId>;
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct UnlockChunk<Balance> {
	pub value: Balance,
	pub era: EraIndex,
	pub claimed: Balance,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, Default)]
pub struct StakingLedger<Balance> {
	pub total_bonded: Balance,
	pub active: Balance,
	pub unlocking: Vec<UnlockChunk<Balance>>,
	pub free: Balance,
}

impl<Balance> StakingLedger<Balance>
where
	Balance: HasCompact + Copy + Saturating + AtLeast32Bit,
{
	/// Remove entries from `unlocking` that are sufficiently old and reduce the
	/// total by the sum of their balances, and increase the free by the sum of
	/// their claimed.
	fn consolidate_unlocked(self, current_era: EraIndex) -> Self {
		let mut total_bonded = self.total_bonded;
		let mut free = self.free;
		let unlocking = self
			.unlocking
			.into_iter()
			.filter(|chunk| {
				if chunk.era > current_era {
					true
				} else {
					total_bonded = total_bonded.saturating_sub(chunk.value);
					free = free.saturating_add(chunk.claimed);
					false
				}
			})
			.collect();

		Self {
			total_bonded: total_bonded,
			active: self.active,
			unlocking: unlocking,
			free: free,
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

pub trait PolkadotBridgeType<BlockNumber> {
	type BondingDuration: Get<EraIndex>;
	type EraLength: Get<BlockNumber>;
	type PolkadotAccountId: Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeDisplay + Ord + Default;
}

pub trait PolkadotBridgeCall<BlockNumber, Balance>: PolkadotBridgeType<BlockNumber> {
	fn bond_extra(amount: Balance);
	fn unbond(amount: Balance);
	fn rebond(amount: Balance);
	fn withdraw_unbonded();
	fn nominate(targets: Vec<Self::PolkadotAccountId>);
	fn transfer(to: Self::PolkadotAccountId, amount: Balance);
	fn payout_nominator();
}

pub trait PolkadotBridgeState<Balance> {
	fn ledger() -> StakingLedger<Balance>;
	fn balance() -> Balance;
	fn current_era() -> EraIndex;
}

pub trait PolkadotBridge<BlockNumber, Balance>:
	PolkadotBridgeCall<BlockNumber, Balance> + PolkadotBridgeState<Balance>
{
}

pub trait OnCommission<Balance> {
	fn on_commission(amount: Balance);
}
