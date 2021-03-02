//! RPC interface for the staking pool module.

use codec::Codec;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use module_staking_pool_rpc_runtime_api::BalanceInfo;
use module_support::ExchangeRate;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{
	generic::BlockId,
	traits::{Block as BlockT, MaybeDisplay, MaybeFromStr},
};
use std::sync::Arc;

pub use self::gen_client::Client as StakingPoolClient;
pub use module_staking_pool_rpc_runtime_api::StakingPoolApi as StakingPoolRuntimeApi;

#[rpc]
pub trait StakingPoolApi<BlockHash, AccountId, ResponseType> {
	#[rpc(name = "stakingPool_getAvailableUnbonded")]
	fn get_available_unbonded(&self, account: AccountId, at: Option<BlockHash>) -> Result<ResponseType>;

	#[rpc(name = "stakingPool_getLiquidStakingExchangeRate")]
	fn get_liquid_staking_exchange_rate(&self, at: Option<BlockHash>) -> Result<ExchangeRate>;
}

/// A struct that implements the [`StakingPoolApi`].
pub struct StakingPool<C, B> {
	client: Arc<C>,
	_marker: std::marker::PhantomData<B>,
}

impl<C, B> StakingPool<C, B> {
	/// Create new `StakingPool` with the given reference to the client.
	pub fn new(client: Arc<C>) -> Self {
		StakingPool {
			client,
			_marker: Default::default(),
		}
	}
}

pub enum Error {
	RuntimeError,
}

impl From<Error> for i64 {
	fn from(e: Error) -> i64 {
		match e {
			Error::RuntimeError => 1,
		}
	}
}

impl<C, Block, AccountId, Balance> StakingPoolApi<<Block as BlockT>::Hash, AccountId, BalanceInfo<Balance>>
	for StakingPool<C, Block>
where
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: StakingPoolRuntimeApi<Block, AccountId, Balance>,
	AccountId: Codec,
	Balance: Codec + MaybeDisplay + MaybeFromStr,
{
	fn get_available_unbonded(
		&self,
		account: AccountId,
		at: Option<<Block as BlockT>::Hash>,
	) -> Result<BalanceInfo<Balance>> {
		let api = self.client.runtime_api();
		let at = BlockId::hash(at.unwrap_or(
			// If the block hash is not supplied assume the best block.
			self.client.info().best_hash,
		));

		api.get_available_unbonded(&at, account).map_err(|e| RpcError {
			code: ErrorCode::ServerError(Error::RuntimeError.into()),
			message: "Unable to get available unbonded.".into(),
			data: Some(format!("{:?}", e).into()),
		})
	}

	fn get_liquid_staking_exchange_rate(&self, at: Option<<Block as BlockT>::Hash>) -> Result<ExchangeRate> {
		let api = self.client.runtime_api();
		let at = BlockId::hash(at.unwrap_or(
			// If the block hash is not supplied assume the best block.
			self.client.info().best_hash,
		));

		api.get_liquid_staking_exchange_rate(&at).map_err(|e| RpcError {
			code: ErrorCode::ServerError(Error::RuntimeError.into()),
			message: "Unable to get liquid staking exchange rate.".into(),
			data: Some(format!("{:?}", e).into()),
		})
	}
}
