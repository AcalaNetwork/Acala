use grandpa_primitives::AuthorityId as GrandpaId;
use hex_literal::hex;
use runtime::{
	AccountId, AuraConfig, BalancesConfig, CurrencyId, GenesisConfig, GrandpaConfig, IndicesConfig,
	OperatorMembershipConfig, Signature, SudoConfig, SystemConfig, TokensConfig, WASM_BINARY,
};
use sc_service;
use sc_telemetry::TelemetryEndpoints;
use serde_json::map::Map;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{crypto::UncheckedInto, sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};

// Note this is the URL for the telemetry server
//const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::ChainSpec<GenesisConfig>;

/// The chain specification option. This is expected to come in from the CLI and
/// is little more than one of a number of alternatives which can easily be converted
/// from a string (`--chain=...`) into a `ChainSpec`.
#[derive(Clone, Debug)]
pub enum Alternative {
	/// Whatever the current runtime is, with just Alice as an auth.
	Development,
	/// Whatever the current runtime is, with simple Alice/Bob auths.
	LocalTestnet,
	AlphaTestnet,
	AlphaTestnetLatest,
}

type AccountPublic = <Signature as Verify>::Signer;

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

/// Helper function to generate session key from seed
pub fn get_authority_keys_from_seed(seed: &str) -> (AuraId, GrandpaId) {
	(get_from_seed::<AuraId>(seed), get_from_seed::<GrandpaId>(seed))
}

impl Alternative {
	/// Get an actual chain config from one of the alternatives.
	pub(crate) fn load(self) -> Result<ChainSpec, String> {
		let mut properties = Map::new();
		properties.insert("tokenSymbol".into(), "ACA".into());
		properties.insert("tokenDecimals".into(), 18.into());

		Ok(match self {
			Alternative::Development => ChainSpec::from_genesis(
				"Development",
				"dev",
				|| {
					testnet_genesis(
						vec![get_authority_keys_from_seed("Alice")],
						get_account_id_from_seed::<sr25519::Public>("Alice"),
						vec![
							get_account_id_from_seed::<sr25519::Public>("Alice"),
							get_account_id_from_seed::<sr25519::Public>("Bob"),
							get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
							get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
						],
					)
				},
				vec![],
				None,
				None,
				Some(properties),
				None,
			),
			Alternative::LocalTestnet => ChainSpec::from_genesis(
				"Local Testnet",
				"local_testnet",
				|| {
					testnet_genesis(
						vec![
							get_authority_keys_from_seed("Alice"),
							get_authority_keys_from_seed("Bob"),
						],
						get_account_id_from_seed::<sr25519::Public>("Alice"),
						vec![
							get_account_id_from_seed::<sr25519::Public>("Alice"),
							get_account_id_from_seed::<sr25519::Public>("Bob"),
							get_account_id_from_seed::<sr25519::Public>("Charlie"),
							get_account_id_from_seed::<sr25519::Public>("Dave"),
							get_account_id_from_seed::<sr25519::Public>("Eve"),
							get_account_id_from_seed::<sr25519::Public>("Ferdie"),
							get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
							get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
							get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
							get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
							get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
							get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
						],
					)
				},
				vec![],
				None,
				None,
				Some(properties),
				None,
			),
			Alternative::AlphaTestnet => ChainSpec::from_json_bytes(&include_bytes!("../resources/alpha.json")[..])?,
			Alternative::AlphaTestnetLatest => {
				ChainSpec::from_genesis(
					"Acala",
					"acala",
					|| {
						// SECRET="..."
						// ./target/debug/subkey --sr25519 inspect "$SECRET//acala//aura"
						// ./target/debug/subkey --ed25519 inspect "$SECRET//acala//grandpa"
						// ./target/debug/subkey inspect "$SECRET//acala//root"
						// ./target/debug/subkey inspect "$SECRET//acala//oracle"
						alphanet_genesis(
							vec![(
								// 5D2Nr1DsxqWDwAf84pWavtCnkysfE9gjzpDJMbD6ncsCwg8d
								hex!["2a75be90e325f6251be9880b1268ab21ef65bb950ac77a21298a81548f9e435d"]
									.unchecked_into(),
								// 5EWtr28JevMKMwtriEAVebhgwd6iSqGcpPDsHeVhVs3if9Po
								hex!["6c71d6cdf562a68345b4294eb9aad46599ff74fe6dc1a415f10e0fe2843cea3a"]
									.unchecked_into(),
							)],
							// 5F98oWfz2r5rcRVnP9VCndg33DAAsky3iuoBSpaPUbgN9AJn
							hex!["8815a8024b06a5b4c8703418f52125c923f939a5c40a717f6ae3011ba7719019"].into(),
							vec![
								// 5F98oWfz2r5rcRVnP9VCndg33DAAsky3iuoBSpaPUbgN9AJn
								hex!["8815a8024b06a5b4c8703418f52125c923f939a5c40a717f6ae3011ba7719019"].into(),
								// 5Fe3jZRbKes6aeuQ6HkcTvQeNhkkRPTXBwmNkuAPoimGEv45
								hex!["9e22b64c980329ada2b46a783623bcf1f1d0418f6a2b5fbfb7fb68dbac5abf0f"].into(),
							],
						)
					},
					vec![
						"/dns4/testnet-bootnode-1.acala.laminar.one/tcp/30333/p2p/QmfZFm6bGGpaJ8J2TJb14ubtm86hdNeAwqvxDoVw5FDWUC".into(),
					],
					Some(TelemetryEndpoints::new(vec![(
						"wss://telemetry.polkadot.io/submit/".into(),
						0,
					)])),
					Some("acala"),
					Some(properties),
					None,
				)
			}
		})
	}

	pub(crate) fn from(s: &str) -> Option<Self> {
		match s {
			"dev" => Some(Alternative::Development),
			"local" => Some(Alternative::LocalTestnet),
			"" | "alpha" => Some(Alternative::AlphaTestnet),
			"alpha-latest" => Some(Alternative::AlphaTestnetLatest),
			_ => None,
		}
	}
}

const INITIAL_BALANCE: u128 = 1_000_000_000_000_000_000_000_u128; // $1M

fn testnet_genesis(
	initial_authorities: Vec<(AuraId, GrandpaId)>,
	root_key: AccountId,
	endowed_accounts: Vec<AccountId>,
) -> GenesisConfig {
	GenesisConfig {
		system: Some(SystemConfig {
			code: WASM_BINARY.to_vec(),
			changes_trie_config: Default::default(),
		}),
		pallet_indices: Some(IndicesConfig {
			ids: endowed_accounts.clone(),
		}),
		pallet_balances: Some(BalancesConfig {
			balances: endowed_accounts.iter().cloned().map(|k| (k, INITIAL_BALANCE)).collect(),
			vesting: vec![],
		}),
		pallet_sudo: Some(SudoConfig { key: root_key.clone() }),
		pallet_aura: Some(AuraConfig {
			authorities: initial_authorities.iter().map(|x| (x.0.clone())).collect(),
		}),
		pallet_grandpa: Some(GrandpaConfig {
			authorities: initial_authorities.iter().map(|x| (x.1.clone(), 1)).collect(),
		}),
		pallet_collective_Instance1: Some(Default::default()),
		pallet_membership_Instance1: Some(OperatorMembershipConfig {
			members: vec![root_key],
			phantom: Default::default(),
		}),
		orml_tokens: Some(TokensConfig {
			endowed_accounts: endowed_accounts
				.iter()
				.flat_map(|x| {
					vec![
						(x.clone(), CurrencyId::DOT, INITIAL_BALANCE),
						(x.clone(), CurrencyId::XBTC, INITIAL_BALANCE),
					]
				})
				.collect(),
		}),
	}
}

fn alphanet_genesis(
	initial_authorities: Vec<(AuraId, GrandpaId)>,
	root_key: AccountId,
	endowed_accounts: Vec<AccountId>,
) -> GenesisConfig {
	GenesisConfig {
		system: Some(SystemConfig {
			code: WASM_BINARY.to_vec(),
			changes_trie_config: Default::default(),
		}),
		pallet_indices: Some(IndicesConfig {
			ids: endowed_accounts.clone(),
		}),
		pallet_balances: Some(BalancesConfig {
			balances: endowed_accounts.iter().cloned().map(|k| (k, INITIAL_BALANCE)).collect(),
			vesting: vec![],
		}),
		pallet_sudo: Some(SudoConfig { key: root_key.clone() }),
		pallet_aura: Some(AuraConfig {
			authorities: initial_authorities.iter().map(|x| (x.0.clone())).collect(),
		}),
		pallet_grandpa: Some(GrandpaConfig {
			authorities: initial_authorities.iter().map(|x| (x.1.clone(), 1)).collect(),
		}),
		pallet_collective_Instance1: Some(Default::default()),
		pallet_membership_Instance1: Some(OperatorMembershipConfig {
			members: vec![root_key],
			phantom: Default::default(),
		}),
		orml_tokens: Some(TokensConfig {
			endowed_accounts: endowed_accounts
				.iter()
				.flat_map(|x| {
					vec![
						(x.clone(), CurrencyId::DOT, INITIAL_BALANCE),
						(x.clone(), CurrencyId::XBTC, INITIAL_BALANCE),
					]
				})
				.collect(),
		}),
	}
}
