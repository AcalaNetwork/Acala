#![warn(missing_docs)]

use std::sync::Arc;

use runtime::{opaque::Block, AccountId, Balance, CurrencyId, Index, TimeStampedPrice, UncheckedExtrinsic};
use sp_api::ProvideRuntimeApi;
use sp_transaction_pool::TransactionPool;

/// A type representing all RPC extensions.
pub type RpcExtension = jsonrpc_core::IoHandler<sc_rpc::Metadata>;

/// Instantiate all RPC extensions.
pub fn create_full<C, P>(client: Arc<C>, pool: Arc<P>) -> RpcExtension
where
	C: ProvideRuntimeApi<Block>,
	C: sc_client::blockchain::HeaderBackend<Block>,
	C: Send + Sync + 'static,
	C::Api: frame_rpc_system::AccountNonceApi<Block, AccountId, Index>,
	C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance, UncheckedExtrinsic>,
	C::Api: orml_oracle_rpc::OracleRuntimeApi<Block, CurrencyId, TimeStampedPrice>,
	P: TransactionPool + Sync + Send + 'static,
{
	use frame_rpc_system::{FullSystem, SystemApi};
	use orml_oracle_rpc::{Oracle, OracleApi};
	use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApi};

	let mut io = jsonrpc_core::IoHandler::default();
	io.extend_with(SystemApi::to_delegate(FullSystem::new(client.clone(), pool)));
	io.extend_with(TransactionPaymentApi::to_delegate(TransactionPayment::new(
		client.clone(),
	)));
	io.extend_with(OracleApi::to_delegate(Oracle::new(client)));
	io
}

/// Instantiate all RPC extensions for light node.
pub fn create_light<C, P, F>(
	client: Arc<C>,
	remote_blockchain: Arc<dyn sc_client::light::blockchain::RemoteBlockchain<Block>>,
	fetcher: Arc<F>,
	pool: Arc<P>,
) -> RpcExtension
where
	C: ProvideRuntimeApi<Block>,
	C: sc_client::blockchain::HeaderBackend<Block>,
	C: Send + Sync + 'static,
	C::Api: frame_rpc_system::AccountNonceApi<Block, AccountId, Index>,
	C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance, UncheckedExtrinsic>,
	P: TransactionPool + Sync + Send + 'static,
	F: sc_client::light::fetcher::Fetcher<Block> + 'static,
{
	use frame_rpc_system::{LightSystem, SystemApi};

	let mut io = jsonrpc_core::IoHandler::default();
	io.extend_with(SystemApi::<AccountId, Index>::to_delegate(LightSystem::new(
		client,
		remote_blockchain,
		fetcher,
		pool,
	)));
	io
}
