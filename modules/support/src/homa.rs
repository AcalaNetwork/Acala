use codec::{Decode, Encode, HasCompact};
use frame_support::{traits::Get, Parameter};
use rstd::fmt::Debug;
use rstd::prelude::*;
use sp_runtime::{
	traits::{MaybeDisplay, MaybeSerializeDeserialize, Member},
	DispatchError, DispatchResult, RuntimeDebug,
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
	pub value: Balance,
	pub era: EraIndex,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, Default)]
pub struct PolkadotStakingLedger<Balance> {
	pub total: Balance,
	pub active: Balance,
	pub unlocking: Vec<PolkadotUnlockChunk<Balance>>,
}

pub trait PolkadotBridgeType<BlockNumber> {
	type BondingDuration: Get<EraIndex>;
	type EraLength: Get<BlockNumber>;
	type PolkadotAccountId: Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeDisplay + Ord + Default;
}

pub trait PolkadotBridgeCall<BlockNumber, Balance, AccountId>: PolkadotBridgeType<BlockNumber> {
	fn bond_extra(amount: Balance) -> DispatchResult;
	fn unbond(amount: Balance) -> DispatchResult;
	fn rebond(amount: Balance) -> DispatchResult;
	fn withdraw_unbonded();
	fn nominate(targets: Vec<Self::PolkadotAccountId>);
	fn transfer_to_bridge(from: &AccountId, amount: Balance) -> DispatchResult;
	fn receive_from_bridge(to: &AccountId, amount: Balance) -> DispatchResult;
	fn payout_nominator();
}

pub trait PolkadotBridgeState<Balance> {
	fn ledger() -> PolkadotStakingLedger<Balance>;
	fn balance() -> Balance;
	fn current_era() -> EraIndex;
}

pub trait PolkadotBridge<BlockNumber, Balance, AccountId>:
	PolkadotBridgeCall<BlockNumber, Balance, AccountId> + PolkadotBridgeState<Balance>
{
}

pub trait OnCommission<Balance, CurrencyId> {
	fn on_commission(currency_id: CurrencyId, amount: Balance);
}

impl<Balance, CurrencyId> OnCommission<Balance, CurrencyId> for () {
	fn on_commission(_currency_id: CurrencyId, _amount: Balance) {}
}

pub trait HomaProtocol<AccountId> {
	type Balance: Decode + Encode + Debug + Eq + PartialEq + Clone + HasCompact;

	fn mint(who: &AccountId, amount: Self::Balance) -> rstd::result::Result<Self::Balance, DispatchError>;
	fn redeem_by_unbond(who: &AccountId, amount: Self::Balance) -> DispatchResult;
	fn redeem_by_free_unbonded(who: &AccountId, amount: Self::Balance) -> DispatchResult;
	fn redeem_by_claim_unbonding(who: &AccountId, amount: Self::Balance, target_era: EraIndex) -> DispatchResult;
	fn withdraw_redemption(who: &AccountId) -> rstd::result::Result<Self::Balance, DispatchError>;
}
