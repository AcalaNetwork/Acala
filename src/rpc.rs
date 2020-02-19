#![warn(missing_docs)]

use std::{fmt, sync::Arc};

use frame_rpc_system::AccountNonceApi;
use runtime::{opaque::Block, AccountId, Balance, CurrencyId, Index, TimeStampedPrice, UncheckedExtrinsic};
use sc_client::blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sc_consensus_babe::{Config, Epoch};
use sc_consensus_babe_rpc::BabeRPCHandler;
use sc_consensus_epochs::SharedEpochChanges;
use sc_keystore::KeyStorePtr;
use sp_api::ProvideRuntimeApi;
use sp_consensus::SelectChain;
use sp_consensus_babe::BabeApi;
use sp_transaction_pool::TransactionPool;

/// Light client extra dependencies.
pub struct LightDeps<C, F, P> {
	/// The client instance to use.
	pub client: Arc<C>,
	/// Transaction pool instance.
	pub pool: Arc<P>,
	/// Remote access to the blockchain (async).
	pub remote_blockchain: Arc<dyn sc_client::light::blockchain::RemoteBlockchain<Block>>,
	/// Fetcher instance.
	pub fetcher: Arc<F>,
}

/// Extra dependencies for BABE.
pub struct BabeDeps {
	/// BABE protocol config.
	pub babe_config: Config,
	/// BABE pending epoch changes.
	pub shared_epoch_changes: SharedEpochChanges<Block, Epoch>,
	/// The keystore that manages the keys of the node.
	pub keystore: KeyStorePtr,
}

/// Full client dependencies.
pub struct FullDeps<C, P, SC> {
	/// The client instance to use.
	pub client: Arc<C>,
	/// Transaction pool instance.
	pub pool: Arc<P>,
	/// The SelectChain Strategy
	pub select_chain: SC,
	/// BABE specific dependencies.
	pub babe: BabeDeps,
}

/// Instantiate all Full RPC extensions.
pub fn create_full<C, P, M, SC>(deps: FullDeps<C, P, SC>) -> jsonrpc_core::IoHandler<M>
where
	C: ProvideRuntimeApi<Block>,
	C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError> + 'static,
	C: Send + Sync + 'static,
	C::Api: AccountNonceApi<Block, AccountId, Index>,
	C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance, UncheckedExtrinsic>,
	C::Api: BabeApi<Block>,
	C::Api: orml_oracle_rpc::OracleRuntimeApi<Block, CurrencyId, TimeStampedPrice>,
	<C::Api as sp_api::ApiErrorExt>::Error: fmt::Debug,
	P: TransactionPool + 'static,
	M: jsonrpc_core::Metadata + Default,
	SC: SelectChain<Block> + 'static,
{
	use frame_rpc_system::{FullSystem, SystemApi};
	use orml_oracle_rpc::{Oracle, OracleApi};
	use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApi};

	let mut io = jsonrpc_core::IoHandler::default();
	let FullDeps {
		client,
		pool,
		select_chain,
		babe,
	} = deps;
	let BabeDeps {
		keystore,
		babe_config,
		shared_epoch_changes,
	} = babe;

	io.extend_with(SystemApi::to_delegate(FullSystem::new(client.clone(), pool)));
	io.extend_with(TransactionPaymentApi::to_delegate(TransactionPayment::new(
		client.clone(),
	)));
	io.extend_with(sc_consensus_babe_rpc::BabeApi::to_delegate(BabeRPCHandler::new(
		client.clone(),
		shared_epoch_changes,
		keystore,
		babe_config,
		select_chain,
	)));
	io.extend_with(OracleApi::to_delegate(Oracle::new(client)));

	io
}

/// Instantiate all Light RPC extensions.
pub fn create_light<C, P, M, F>(deps: LightDeps<C, F, P>) -> jsonrpc_core::IoHandler<M>
where
	C: HeaderBackend<Block>,
	C: Send + Sync + 'static,
	F: sc_client::light::fetcher::Fetcher<Block> + 'static,
	P: TransactionPool + 'static,
	M: jsonrpc_core::Metadata + Default,
{
	use frame_rpc_system::{LightSystem, SystemApi};

	let LightDeps {
		client,
		pool,
		remote_blockchain,
		fetcher,
	} = deps;
	let mut io = jsonrpc_core::IoHandler::default();
	io.extend_with(SystemApi::<AccountId, Index>::to_delegate(LightSystem::new(
		client,
		remote_blockchain,
		fetcher,
		pool,
	)));

	io
}
