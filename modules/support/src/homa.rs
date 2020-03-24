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

#[impl_trait_for_tuples::impl_for_tuples(30)]
pub trait OnNewEra {
	fn on_new_era(era: EraIndex);
}

pub trait NomineesProvider<AccountId> {
	fn nominees() -> Vec<AccountId>;
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct PolkadotUnlockChunk<Balance> {
	#[codec(compact)]
	pub value: Balance,
	#[codec(compact)]
	pub era: EraIndex,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, Default)]
pub struct PolkadotStakingLedger<Balance> {
	#[codec(compact)]
	pub total: Balance,
	#[codec(compact)]
	pub active: Balance,
	pub unlocking: Vec<PolkadotUnlockChunk<Balance>>,
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
	fn ledger() -> PolkadotStakingLedger<Balance>;
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
