use aura_primitives::sr25519::AuthorityId as AuraId;
use grandpa_primitives::AuthorityId as GrandpaId;
use hex_literal::hex;
use primitives::{crypto::UncheckedInto, sr25519, Pair, Public};
use runtime::{
	AccountId, AuraConfig, BalancesConfig, CurrencyId, GenesisConfig, GrandpaConfig, IndicesConfig,
	OperatorMembershipConfig, Signature, SudoConfig, SystemConfig, TokensConfig, WASM_BINARY,
};
use serde_json::map::Map;
use sr_primitives::traits::{IdentifyAccount, Verify};
use substrate_service;
use substrate_telemetry::TelemetryEndpoints;

// Note this is the URL for the telemetry server
//const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = substrate_service::ChainSpec<GenesisConfig>;

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

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

type AccountPublic = <Signature as Verify>::Signer;

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Helper function to generate an authority key for Aura
pub fn get_authority_keys_from_seed(s: &str) -> (AuraId, GrandpaId) {
	(get_from_seed::<AuraId>(s), get_from_seed::<GrandpaId>(s))
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
						// ./target/debug/subkey --ed25519 inspect "$SECRET//acala//aura"
						// ./target/debug/subkey --ed25519 inspect "$SECRET//acala//grandpa"
						// ./target/debug/subkey inspect "$SECRET//acala//root"
						alphanet_genesis(
							vec![(
								// 5GrF4EsvdGLba46WmPS7YYvt49F3kDrkJNuTcUjKPhpzkWYM
								hex!["d3ac01000fa51af509d12586847013aaa0b7ce6cea501745d8190c4d622324f6"]
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
							],
						)
					},
					vec![
						"/dns4/bootnode-1.alpha.acala.network/tcp/30333/p2p/QmdjfMKngmW5BxSg8FqTDuqyBD3NkFFkwZ4BVqjKfCMdWg".into(),
						"/dns4/bootnode-2.alpha.acala.network/tcp/30333/p2p/QmdjfMKngmW5BxSg8FqTDuqyBD3NkFFkwZ4BVqjKfCMdWg".into(),
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
			balances: endowed_accounts.iter().cloned().map(|k| (k, 1 << 60)).collect(),
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
			tokens: vec![CurrencyId::DOT, CurrencyId::XBTC],
			initial_balance: 1_000_000_000_000_000_000_000_u128, // $1M
			endowed_accounts: endowed_accounts.clone(),
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
			balances: endowed_accounts.iter().cloned().map(|k| (k, 1 << 60)).collect(),
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
			tokens: vec![CurrencyId::DOT, CurrencyId::XBTC],
			initial_balance: 1_000_000_000_000_000_000_000_u128, // $1M
			endowed_accounts: endowed_accounts.clone(),
		}),
	}
}
