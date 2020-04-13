use hex_literal::hex;
use module_support::Rate;
use orml_utilities::FixedU128;
use runtime::{
	opaque::Block, opaque::SessionKeys, AccountId, BabeConfig, BalancesConfig, CdpEngineConfig, CdpTreasuryConfig,
	CurrencyId, DexConfig, FinancialCouncilMembershipConfig, GeneralCouncilMembershipConfig, GenesisConfig,
	GrandpaConfig, IndicesConfig, OperatorMembershipConfig, PolkadotBridgeConfig, SessionConfig, Signature,
	StakerStatus, StakingConfig, SudoConfig, SystemConfig, TokensConfig, CENTS, DOLLARS, WASM_BINARY,
};
use sc_chain_spec::ChainSpecExtension;
use sc_service;
use sc_telemetry::TelemetryEndpoints;
use serde::{Deserialize, Serialize};
use serde_json::map::Map;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_core::{crypto::UncheckedInto, sr25519, Pair, Public};
use sp_finality_grandpa::AuthorityId as GrandpaId;
use sp_runtime::traits::{IdentifyAccount, Verify};
use sp_runtime::Perbill;

// Note this is the URL for the telemetry server
//const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

/// Node `ChainSpec` extensions.
///
/// Additional parameters for some Substrate core modules,
/// customizable from the chain spec.
#[derive(Default, Clone, Serialize, Deserialize, ChainSpecExtension)]
#[serde(rename_all = "camelCase")]
pub struct Extensions {
	/// Block numbers with known hashes.
	pub fork_blocks: sc_client::ForkBlocks<Block>,
	/// Known bad block hashes.
	pub bad_blocks: sc_client::BadBlocks<Block>,
}

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<GenesisConfig, Extensions>;

/// The chain specification option. This is expected to come in from the CLI and
/// is little more than one of a number of alternatives which can easily be converted
/// from a string (`--chain=...`) into a `ChainSpec`.
#[derive(Clone, Debug)]
pub enum Alternative {
	/// Whatever the current runtime is, with just Alice as an auth.
	Development,
	/// Whatever the current runtime is, with simple Alice/Bob auths.
	LocalTestnet,
	MandalaTestnet,
	MandalaTestnetLatest,
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
pub fn get_authority_keys_from_seed(seed: &str) -> (AccountId, AccountId, GrandpaId, BabeId) {
	(
		get_account_id_from_seed::<sr25519::Public>(&format!("{}//stash", seed)),
		get_account_id_from_seed::<sr25519::Public>(seed),
		get_from_seed::<GrandpaId>(seed),
		get_from_seed::<BabeId>(seed),
	)
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
				Default::default(),
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
				Default::default(),
			),
			Alternative::MandalaTestnet => {
				ChainSpec::from_json_bytes(&include_bytes!("../resources/mandala-dist.json")[..])?
			}
			Alternative::MandalaTestnetLatest => {
				ChainSpec::from_genesis(
					"Acala Mandala TC2",
					"mandala22",
					|| {
						// SECRET="..."
						// ./target/debug/subkey inspect "$SECRET//acala//root"
						// ./target/debug/subkey --ed25519 inspect "$SECRET//acala//oracle"
						// ./target/debug/subkey --sr25519 inspect "$SECRET//acala//1//validator"
						// ./target/debug/subkey --sr25519 inspect "$SECRET//acala//1//babe"
						// ./target/debug/subkey --ed25519 inspect "$SECRET//acala//1//grandpa"
						// ./target/debug/subkey --sr25519 inspect "$SECRET//acala//2//validator"
						// ./target/debug/subkey --sr25519 inspect "$SECRET//acala//2//babe"
						// ./target/debug/subkey --ed25519 inspect "$SECRET//acala//2//grandpa"
						// ./target/debug/subkey --sr25519 inspect "$SECRET//acala//3//validator"
						// ./target/debug/subkey --sr25519 inspect "$SECRET//acala//3//babe"
						// ./target/debug/subkey --ed25519 inspect "$SECRET//acala//3//grandpa"
						mandala_genesis(
							vec![
								(
									// 5CLg63YpPJNqcyWaYebk3LuuUVp3un7y1tmuV3prhdbnMA77
									hex!["0c2df85f943312fc853059336627d0b7a08669629ebd99b4debc6e58c1b35c2b"].into(),
									hex!["0c2df85f943312fc853059336627d0b7a08669629ebd99b4debc6e58c1b35c2b"].into(),
									hex!["21b5a771b99ef0f059c476502c018c4b817fb0e48858e95a238850d2b7828556"]
										.unchecked_into(),
									hex!["948f15728a5fd66e36503c048cc7b448cb360a825240c48ff3f89efe050de608"]
										.unchecked_into(),
								),
								(
									// 5FnLzAUmXeTZg5J9Ao5psKU68oA5PBekXqhrZCKDbhSCQi88
									hex!["a476c0050065dafac1e9ff7bf602fe628ceadacf67650f8317554bd571b73507"].into(),
									hex!["a476c0050065dafac1e9ff7bf602fe628ceadacf67650f8317554bd571b73507"].into(),
									hex!["77f3c27e98da7849ed0749e1dea449321a4a5a36a1dccf3f08fc0ab3af24c62e"]
										.unchecked_into(),
									hex!["b4f5713322656d29930aa89efa5509554a36c40fb50a226eae0f38fc1a6ceb25"]
										.unchecked_into(),
								),
								(
									// 5Gn5LuLuWNcY21Vue4QcFFD3hLvjQY3weMHXuEyejUbUnArt
									hex!["d07e538fee7c42be9b2627ea5caac9a30f1869d65af2a19df70138d5fcc34310"].into(),
									hex!["d07e538fee7c42be9b2627ea5caac9a30f1869d65af2a19df70138d5fcc34310"].into(),
									hex!["c5dfcf68ccf1a64ed4145383e4bbbb8bbcc50f654d87187c39df2b88a9683b7f"]
										.unchecked_into(),
									hex!["4cc54799f38715771605a21e8272a7a1344667e4681611988a913412755a8a04"]
										.unchecked_into(),
								),
							],
							// 5F98oWfz2r5rcRVnP9VCndg33DAAsky3iuoBSpaPUbgN9AJn
							hex!["8815a8024b06a5b4c8703418f52125c923f939a5c40a717f6ae3011ba7719019"].into(),
							vec![
								// 5F98oWfz2r5rcRVnP9VCndg33DAAsky3iuoBSpaPUbgN9AJn
								hex!["8815a8024b06a5b4c8703418f52125c923f939a5c40a717f6ae3011ba7719019"].into(),
								// 5GeTpaLR637ztQqFvwCZocZhLp1QqHURKH6Gj7CZteRCAhMs
								hex!["cab00722883a824e7fc368ff2ad53ffcce3fa3b794080311218bee8e902929df"].into(),
							],
						)
					},
					vec![
						"/dns4/testnet-bootnode-1.acala.laminar.one/tcp/30333/p2p/QmYmd7hdwanKpB5jVp6VndHcgjcSYq9izU8ZzccnMWYhoA".into(),
					],
					Some(TelemetryEndpoints::new(vec![(
						"wss://telemetry.polkadot.io/submit/".into(),
						0,
					)])),
					Some("mandala2"),
					Some(properties),
					Default::default(),
				)
			}
		})
	}

	pub(crate) fn from(s: &str) -> Option<Self> {
		match s {
			"dev" => Some(Alternative::Development),
			"local" => Some(Alternative::LocalTestnet),
			"" | "mandala" => Some(Alternative::MandalaTestnet),
			"mandala-latest" => Some(Alternative::MandalaTestnetLatest),
			_ => None,
		}
	}
}

fn session_keys(grandpa: GrandpaId, babe: BabeId) -> SessionKeys {
	SessionKeys { grandpa, babe }
}

const INITIAL_BALANCE: u128 = 1_000_000 * DOLLARS;
const INITIAL_STAKING: u128 = 100_000 * DOLLARS;

fn testnet_genesis(
	initial_authorities: Vec<(AccountId, AccountId, GrandpaId, BabeId)>,
	root_key: AccountId,
	endowed_accounts: Vec<AccountId>,
) -> GenesisConfig {
	GenesisConfig {
		system: Some(SystemConfig {
			code: WASM_BINARY.to_vec(),
			changes_trie_config: Default::default(),
		}),
		pallet_indices: Some(IndicesConfig { indices: vec![] }),
		pallet_balances: Some(BalancesConfig {
			balances: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), INITIAL_STAKING))
				.chain(endowed_accounts.iter().cloned().map(|k| (k, INITIAL_BALANCE)))
				.collect(),
		}),
		pallet_session: Some(SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.0.clone(), session_keys(x.2.clone(), x.3.clone())))
				.collect::<Vec<_>>(),
		}),
		pallet_staking: Some(StakingConfig {
			validator_count: initial_authorities.len() as u32 * 2,
			minimum_validator_count: initial_authorities.len() as u32,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), INITIAL_STAKING, StakerStatus::Validator))
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		}),
		pallet_sudo: Some(SudoConfig { key: root_key.clone() }),
		pallet_babe: Some(BabeConfig { authorities: vec![] }),
		pallet_grandpa: Some(GrandpaConfig { authorities: vec![] }),
		pallet_collective_Instance1: Some(Default::default()),
		pallet_membership_Instance1: Some(GeneralCouncilMembershipConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		}),
		pallet_collective_Instance2: Some(Default::default()),
		pallet_membership_Instance2: Some(FinancialCouncilMembershipConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		}),
		pallet_collective_Instance3: Some(Default::default()),
		pallet_membership_Instance3: Some(OperatorMembershipConfig {
			members: vec![root_key],
			phantom: Default::default(),
		}),
		pallet_treasury: Some(Default::default()),
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
		module_cdp_treasury: Some(CdpTreasuryConfig {
			surplus_auction_fixed_size: 1_000 * DOLLARS, // amount in aUSD of per surplus auction
			surplus_buffer_size: 10_000 * DOLLARS,       // cache amount, exceed this will create surplus auction
			initial_amount_per_debit_auction: 2_000 * DOLLARS, // initial bid amount in ACA of per debit auction
			debit_auction_fixed_size: 1_000 * DOLLARS,   // amount in debit(aUSD) of per debit auction
			collateral_auction_maximum_size: vec![
				(CurrencyId::DOT, 1 * DOLLARS), // (currency_id, max size of a collateral auction)
				(CurrencyId::XBTC, 1 * DOLLARS),
			],
		}),
		module_cdp_engine: Some(CdpEngineConfig {
			collaterals_params: vec![
				(
					CurrencyId::DOT,
					Some(FixedU128::from_natural(0)),         // stability fee for this collateral
					Some(FixedU128::from_rational(150, 100)), // liquidation ratio
					Some(FixedU128::from_rational(10, 100)),  // liquidation penalty rate
					Some(FixedU128::from_rational(150, 100)), // required liquidation ratio
					10_000_000 * DOLLARS,                     // maximum debit value in aUSD (cap)
				),
				(
					CurrencyId::XBTC,
					Some(FixedU128::from_natural(0)),
					Some(FixedU128::from_rational(150, 100)),
					Some(FixedU128::from_rational(10, 100)),
					Some(FixedU128::from_rational(150, 100)),
					10_000_000 * DOLLARS,
				),
			],
		}),
		module_dex: Some(DexConfig {
			liquidity_incentive_rate: vec![
				(CurrencyId::DOT, Rate::from_natural(0)),
				(CurrencyId::XBTC, Rate::from_natural(0)),
			],
		}),
		module_polkadot_bridge: Some(PolkadotBridgeConfig {
			mock_reward_rate: FixedU128::from_natural(0),
		}),
	}
}

fn mandala_genesis(
	initial_authorities: Vec<(AccountId, AccountId, GrandpaId, BabeId)>,
	root_key: AccountId,
	endowed_accounts: Vec<AccountId>,
) -> GenesisConfig {
	GenesisConfig {
		system: Some(SystemConfig {
			code: WASM_BINARY.to_vec(),
			changes_trie_config: Default::default(),
		}),
		pallet_indices: Some(IndicesConfig { indices: vec![] }),
		pallet_balances: Some(BalancesConfig {
			balances: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), INITIAL_STAKING))
				.chain(endowed_accounts.iter().cloned().map(|k| (k, INITIAL_BALANCE)))
				.collect(),
		}),
		pallet_session: Some(SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.0.clone(), session_keys(x.2.clone(), x.3.clone())))
				.collect::<Vec<_>>(),
		}),
		pallet_staking: Some(StakingConfig {
			validator_count: 7,
			minimum_validator_count: 3,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), INITIAL_STAKING, StakerStatus::Validator))
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		}),
		pallet_sudo: Some(SudoConfig { key: root_key.clone() }),
		pallet_babe: Some(BabeConfig { authorities: vec![] }),
		pallet_grandpa: Some(GrandpaConfig { authorities: vec![] }),
		pallet_collective_Instance1: Some(Default::default()),
		pallet_membership_Instance1: Some(GeneralCouncilMembershipConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		}),
		pallet_collective_Instance2: Some(Default::default()),
		pallet_membership_Instance2: Some(FinancialCouncilMembershipConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		}),
		pallet_collective_Instance3: Some(Default::default()),
		pallet_membership_Instance3: Some(OperatorMembershipConfig {
			members: endowed_accounts.clone(),
			phantom: Default::default(),
		}),
		pallet_treasury: Some(Default::default()),
		orml_tokens: Some(TokensConfig {
			endowed_accounts: vec![
				(root_key.clone(), CurrencyId::DOT, INITIAL_BALANCE),
				(root_key.clone(), CurrencyId::XBTC, INITIAL_BALANCE),
			],
		}),
		module_cdp_treasury: Some(CdpTreasuryConfig {
			surplus_auction_fixed_size: 100 * DOLLARS, // amount in aUSD of per surplus auction
			surplus_buffer_size: 1_000 * DOLLARS,      // cache amount, exceed this will create surplus auction
			initial_amount_per_debit_auction: 20 * DOLLARS, // initial bid amount in ACA of per debit auction
			debit_auction_fixed_size: 1_000 * DOLLARS, // amount in debit(aUSD) of per debit auction
			collateral_auction_maximum_size: vec![
				(CurrencyId::DOT, 1 * DOLLARS), // (currency_id, max size of a collateral auction)
				(CurrencyId::XBTC, 5 * CENTS),
			],
		}),
		module_cdp_engine: Some(CdpEngineConfig {
			collaterals_params: vec![
				(
					CurrencyId::DOT,
					None,                                     // stability fee for this collateral
					Some(FixedU128::from_rational(105, 100)), // liquidation ratio
					Some(FixedU128::from_rational(3, 100)),   // liquidation penalty rate
					Some(FixedU128::from_rational(110, 100)), // required liquidation ratio
					10_000_000 * DOLLARS,                     // maximum debit value in aUSD (cap)
				),
				(
					CurrencyId::XBTC,
					None,
					Some(FixedU128::from_rational(110, 100)),
					Some(FixedU128::from_rational(4, 100)),
					Some(FixedU128::from_rational(115, 100)),
					10_000_000 * DOLLARS,
				),
			],
		}),
		module_dex: Some(DexConfig {
			liquidity_incentive_rate: vec![
				(CurrencyId::DOT, Rate::from_natural(0)),
				(CurrencyId::XBTC, Rate::from_natural(0)),
			],
		}),
		module_polkadot_bridge: Some(PolkadotBridgeConfig {
			mock_reward_rate: FixedU128::from_natural(0),
		}),
	}
}

pub fn load_spec(id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
	Ok(match Alternative::from(id) {
		Some(spec) => Box::new(spec.load()?),
		None => Box::new(ChainSpec::from_json_file(std::path::PathBuf::from(id))?),
	})
}
