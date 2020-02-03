#![warn(missing_docs)]

use std::sync::Arc;

use runtime::{opaque::Block, AccountId, Balance, CurrencyId, Index, TimeStampedPrice, UncheckedExtrinsic};
use sp_api::ProvideRuntimeApi;
use sp_transaction_pool::TransactionPool;

/// Light client extra dependencies.
pub struct LightDeps<F> {
	/// Remote access to the blockchain (async).
	pub remote_blockchain: Arc<dyn sc_client::light::blockchain::RemoteBlockchain<Block>>,
	/// Fetcher instance.
	pub fetcher: Arc<F>,
}

impl<F> LightDeps<F> {
	/// Create empty `LightDeps` with given `F` type.
	///
	/// This is a convenience method to be used in the service builder,
	/// to make sure the type of the `LightDeps<F>` is matching.
	pub fn none(_: Option<Arc<F>>) -> Option<Self> {
		None
	}
}

/// Instantiate all RPC extensions.
///
/// If you provide `LightDeps`, the system is configured for light client.
pub fn create<C, P, M, F>(client: Arc<C>, pool: Arc<P>, light_deps: Option<LightDeps<F>>) -> jsonrpc_core::IoHandler<M>
where
	C: ProvideRuntimeApi<Block>,
	C: sc_client::blockchain::HeaderBackend<Block>,
	C: Send + Sync + 'static,
	C::Api: frame_rpc_system::AccountNonceApi<Block, AccountId, Index>,
	C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance, UncheckedExtrinsic>,
	C::Api: orml_oracle_rpc::OracleRuntimeApi<Block, CurrencyId, TimeStampedPrice>,
	F: sc_client::light::fetcher::Fetcher<Block> + 'static,
	P: TransactionPool + 'static,
	M: jsonrpc_core::Metadata + Default,
{
	use frame_rpc_system::{FullSystem, LightSystem, SystemApi};
	use orml_oracle_rpc::{Oracle, OracleApi};
	use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApi};

	let mut io = jsonrpc_core::IoHandler::default();

	if let Some(LightDeps {
		remote_blockchain,
		fetcher,
	}) = light_deps
	{
		io.extend_with(SystemApi::<AccountId, Index>::to_delegate(LightSystem::new(
			client,
			remote_blockchain,
			fetcher,
			pool,
		)));
	} else {
		io.extend_with(SystemApi::to_delegate(FullSystem::new(client.clone(), pool)));

		// Making synchronous calls in light client freezes the browser currently,
		// more context: https://github.com/paritytech/substrate/pull/3480
		// These RPCs should use an asynchronous caller instead.
		io.extend_with(TransactionPaymentApi::to_delegate(TransactionPayment::new(
			client.clone(),
		)));

		io.extend_with(OracleApi::to_delegate(Oracle::new(client)));
	}
	io
}
