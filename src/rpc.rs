#![warn(missing_docs)]

use std::sync::Arc;

use runtime::{opaque::Block, AccountId, Balance, CurrencyId, Index, TimeStampedPrice};
pub use sc_rpc_api::DenyUnsafe;
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_transaction_pool::TransactionPool;
use substrate_frame_rpc_system::AccountNonceApi;

/// Full client dependencies.
pub struct FullDeps<C, P> {
	/// The client instance to use.
	pub client: Arc<C>,
	/// Transaction pool instance.
	pub pool: Arc<P>,
	/// Whether to deny unsafe calls
	pub deny_unsafe: DenyUnsafe,
}

/// Instantiate all Full RPC extensions.
pub fn create_full<C, P, M>(deps: FullDeps<C, P>) -> jsonrpc_core::IoHandler<M>
where
	C: ProvideRuntimeApi<Block>,
	C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError> + 'static,
	C: Send + Sync + 'static,
	C::Api: AccountNonceApi<Block, AccountId, Index>,
	C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
	C::Api: orml_oracle_rpc::OracleRuntimeApi<Block, CurrencyId, TimeStampedPrice>,
	C::Api: module_dex_rpc::DexRuntimeApi<Block, CurrencyId, Balance>,
	C::Api: module_staking_pool_rpc::StakingPoolRuntimeApi<Block, AccountId, Balance>,
	C::Api: BlockBuilder<Block>,
	P: TransactionPool + 'static,
	M: jsonrpc_core::Metadata + Default,
{
	use module_dex_rpc::{Dex, DexApi};
	use module_staking_pool_rpc::{StakingPool, StakingPoolApi};
	use orml_oracle_rpc::{Oracle, OracleApi};
	use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApi};
	use substrate_frame_rpc_system::{FullSystem, SystemApi};

	let mut io = jsonrpc_core::IoHandler::default();
	let FullDeps {
		client,
		pool,
		deny_unsafe,
	} = deps;

	io.extend_with(SystemApi::to_delegate(FullSystem::new(
		client.clone(),
		pool,
		deny_unsafe,
	)));
	io.extend_with(TransactionPaymentApi::to_delegate(TransactionPayment::new(
		client.clone(),
	)));
	io.extend_with(OracleApi::to_delegate(Oracle::new(client.clone())));
	io.extend_with(DexApi::to_delegate(Dex::new(client.clone())));
	io.extend_with(StakingPoolApi::to_delegate(StakingPool::new(client)));

	io
}
