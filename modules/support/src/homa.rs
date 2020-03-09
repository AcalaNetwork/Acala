use codec::{Decode, Encode};
use frame_support::{traits::Get, Parameter};
use rstd::fmt::Debug;
use rstd::prelude::*;
use sp_runtime::{
	traits::{MaybeDisplay, MaybeSerializeDeserialize, Member},
	RuntimeDebug,
};

/// Counter for the number of eras that have passed.
pub type EraIndex = u32;

pub trait NomineesProvider<AccountId> {
	fn nominees() -> Vec<AccountId>;
}

pub trait OnNewEra<EraIndex> {
	fn on_new_era(era: EraIndex);
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct UnlockChunk<Balance, EraIndex> {
	value: Balance,
	era: EraIndex,
	claimed: Balance,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, Default)]
pub struct StakingLedger<Balance, EraIndex> {
	pub total_bonded: Balance,
	pub active: Balance,
	pub unlocking: Vec<UnlockChunk<Balance, EraIndex>>,
	pub free: Balance,
}

pub trait PolkadotBridgeType<EraIndex, BlockNumber> {
	type BondingDuration: Get<EraIndex>;
	type EraLength: Get<BlockNumber>;
	type PolkadotAccountId: Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeDisplay + Ord + Default;
}

pub trait PolkadotBridgeCall<EraIndex, BlockNumber, Balance>: PolkadotBridgeType<EraIndex, BlockNumber> {
	fn bond_extra(amount: Balance);
	fn unbond(amount: Balance);
	fn rebond(amount: Balance);
	fn withdraw_unbonded();
	fn nominate(targets: Vec<Self::PolkadotAccountId>);
	fn transfer(to: Self::PolkadotAccountId, amount: Balance);
	fn payout_nominator();
}

pub trait PolkadotBridgeState<Balance, EraIndex> {
	fn ledger() -> StakingLedger<Balance, EraIndex>;
	fn balance() -> Balance;
	fn current_era() -> EraIndex;
}

pub trait PolkadotBridge<EraIndex, BlockNumber, Balance>:
	PolkadotBridgeCall<EraIndex, BlockNumber, Balance> + PolkadotBridgeState<Balance, EraIndex>
{
}
