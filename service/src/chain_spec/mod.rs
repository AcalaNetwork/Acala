//! Acala chain configurations.

use acala_primitives::{AccountId, AccountPublic, Balance, Nonce, PREDEPLOY_ADDRESS_START};
use module_evm::GenesisAccount;
use sc_chain_spec::ChainSpecExtension;
use serde::{Deserialize, Serialize};
use sp_consensus_babe::AuthorityId as BabeId;
use sp_core::{sr25519, Bytes, Pair, Public, H160};
use sp_finality_grandpa::AuthorityId as GrandpaId;
use sp_runtime::traits::IdentifyAccount;
use sp_std::{collections::btree_map::BTreeMap, str::FromStr};

#[cfg(feature = "with-acala-runtime")]
pub mod acala;
#[cfg(feature = "with-karura-runtime")]
pub mod karura;
#[cfg(feature = "with-mandala-runtime")]
pub mod mandala;

// The URL for the telemetry server.
pub const TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

/// Node `ChainSpec` extensions.
///
/// Additional parameters for some Substrate core modules,
/// customizable from the chain spec.
#[derive(Default, Clone, Serialize, Deserialize, ChainSpecExtension)]
#[serde(rename_all = "camelCase")]
pub struct Extensions {
	/// Block numbers with known hashes.
	pub fork_blocks: sc_client_api::ForkBlocks<acala_primitives::Block>,
	/// Known bad block hashes.
	pub bad_blocks: sc_client_api::BadBlocks<acala_primitives::Block>,
}

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Generate an Aura authority key.
pub fn get_authority_keys_from_seed(seed: &str) -> (AccountId, AccountId, GrandpaId, BabeId) {
	(
		get_account_id_from_seed::<sr25519::Public>(&format!("{}//stash", seed)),
		get_account_id_from_seed::<sr25519::Public>(seed),
		get_from_seed::<GrandpaId>(seed),
		get_from_seed::<BabeId>(seed),
	)
}

/// Returns `(evm_genesis_accounts, network_contract_index)`
pub fn evm_genesis() -> (BTreeMap<H160, GenesisAccount<Balance, Nonce>>, u64) {
	let contracts_json = &include_bytes!("../../../predeploy-contracts/resources/bytecodes.json")[..];
	let contracts: Vec<(String, String)> = serde_json::from_slice(contracts_json).unwrap();
	let mut accounts = BTreeMap::new();
	let mut network_contract_index = PREDEPLOY_ADDRESS_START;
	for (_, code_string) in contracts {
		let account = GenesisAccount {
			nonce: 0u32,
			balance: 0u128,
			storage: BTreeMap::new(),
			code: Bytes::from_str(&code_string).unwrap().0,
		};
		let addr = H160::from_low_u64_be(network_contract_index);
		accounts.insert(addr, account);
		network_contract_index += 1;
	}
	(accounts, network_contract_index)
}
