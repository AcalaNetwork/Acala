use super::*;
use frame_support::{traits::Get, Parameter};
use sp_runtime::{
	traits::{MaybeDisplay, MaybeSerializeDeserialize, Member},
	RuntimeDebug,
};

#[impl_trait_for_tuples::impl_for_tuples(30)]
pub trait OnNewEra<EraIndex> {
	fn on_new_era(era: EraIndex);
}

pub trait NomineesProvider<AccountId> {
	fn nominees() -> Vec<AccountId>;
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct PolkadotUnlockChunk<Balance, EraIndex> {
	pub value: Balance,
	pub era: EraIndex,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, Default)]
pub struct PolkadotStakingLedger<Balance, EraIndex> {
	pub total: Balance,
	pub active: Balance,
	pub unlocking: Vec<PolkadotUnlockChunk<Balance, EraIndex>>,
}

pub trait PolkadotBridgeType<BlockNumber, EraIndex> {
	type BondingDuration: Get<EraIndex>;
	type EraLength: Get<BlockNumber>;
	type PolkadotAccountId: Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeDisplay + Ord + Default;
}

pub trait PolkadotBridgeCall<AccountId, BlockNumber, Balance, EraIndex>:
	PolkadotBridgeType<BlockNumber, EraIndex>
{
	fn bond_extra(amount: Balance) -> DispatchResult;
	fn unbond(amount: Balance) -> DispatchResult;
	fn rebond(amount: Balance) -> DispatchResult;
	fn withdraw_unbonded();
	fn nominate(targets: Vec<Self::PolkadotAccountId>);
	fn transfer_to_bridge(from: &AccountId, amount: Balance) -> DispatchResult;
	fn receive_from_bridge(to: &AccountId, amount: Balance) -> DispatchResult;
	fn payout_nominator();
}

pub trait PolkadotBridgeState<Balance, EraIndex> {
	fn ledger() -> PolkadotStakingLedger<Balance, EraIndex>;
	fn balance() -> Balance;
	fn current_era() -> EraIndex;
}

pub trait PolkadotBridge<AccountId, BlockNumber, Balance, EraIndex>:
	PolkadotBridgeCall<AccountId, BlockNumber, Balance, EraIndex> + PolkadotBridgeState<Balance, EraIndex>
{
}

pub trait OnCommission<Balance, CurrencyId> {
	fn on_commission(currency_id: CurrencyId, amount: Balance);
}

impl<Balance, CurrencyId> OnCommission<Balance, CurrencyId> for () {
	fn on_commission(_currency_id: CurrencyId, _amount: Balance) {}
}

pub trait HomaProtocol<AccountId, Balance, EraIndex> {
	type Balance: Decode + Encode + Debug + Eq + PartialEq + Clone + HasCompact;

	fn mint(who: &AccountId, amount: Balance) -> sp_std::result::Result<Balance, DispatchError>;
	fn redeem_by_unbond(who: &AccountId, amount: Balance) -> DispatchResult;
	fn redeem_by_free_unbonded(who: &AccountId, amount: Balance) -> DispatchResult;
	fn redeem_by_claim_unbonding(who: &AccountId, amount: Balance, target_era: EraIndex) -> DispatchResult;
	fn withdraw_redemption(who: &AccountId) -> sp_std::result::Result<Balance, DispatchError>;
}
