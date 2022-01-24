// This file is part of Acala.

// Copyright (C) 2020-2022 Acala Foundation.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use acala_primitives::AccountId;
use hex_literal::hex;
use sc_chain_spec::{ChainType, Properties};
use sc_telemetry::TelemetryEndpoints;
use serde_json::map::Map;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{crypto::UncheckedInto, sr25519};
use sp_runtime::traits::Zero;

use crate::chain_spec::{get_account_id_from_seed, get_parachain_authority_keys_from_seed, Extensions, TELEMETRY_URL};

use acala_runtime::{
	dollar, Balance, BalancesConfig, BlockNumber, CdpEngineConfig, CdpTreasuryConfig, CollatorSelectionConfig,
	DexConfig, FinancialCouncilMembershipConfig, GeneralCouncilMembershipConfig, HomaCouncilMembershipConfig,
	NativeTokenExistentialDeposit, OperatorMembershipAcalaConfig, OrmlNFTConfig, ParachainInfoConfig,
	PolkadotXcmConfig, SS58Prefix, SessionConfig, SessionDuration, SessionKeys, SessionManagerConfig, SudoConfig,
	SystemConfig, TechnicalCommitteeMembershipConfig, TokensConfig, VestingConfig, ACA, AUSD, DOT, LDOT,
};
use runtime_common::TokenInfo;

pub type ChainSpec = sc_service::GenericChainSpec<acala_runtime::GenesisConfig, Extensions>;

pub const PARA_ID: u32 = 2000; // TODO: need confirm

pub fn acala_config() -> Result<ChainSpec, String> {
	ChainSpec::from_json_bytes(&include_bytes!("../../../../resources/acala-dist.json")[..])
}

pub fn wendala_config() -> Result<ChainSpec, String> {
	ChainSpec::from_json_bytes(&include_bytes!("../../../../resources/wendala-dist.json")[..])
}

fn acala_properties() -> Properties {
	let mut properties = Map::new();
	let mut token_symbol: Vec<String> = vec![];
	let mut token_decimals: Vec<u32> = vec![];
	[ACA, AUSD, DOT, LDOT].iter().for_each(|token| {
		token_symbol.push(token.symbol().unwrap().to_string());
		token_decimals.push(token.decimals().unwrap() as u32);
	});
	properties.insert("tokenSymbol".into(), token_symbol.into());
	properties.insert("tokenDecimals".into(), token_decimals.into());
	properties.insert("ss58Format".into(), SS58Prefix::get().into());

	properties
}

pub fn latest_acala_config() -> Result<ChainSpec, String> {
	let wasm_binary = acala_runtime::WASM_BINARY.ok_or("Acala runtime wasm binary not available")?;

	Ok(ChainSpec::from_genesis(
		"Acala",
		"acala",
		ChainType::Live,
		move || {
			let existential_deposit = NativeTokenExistentialDeposit::get();
			let mut total_allocated: Balance = Zero::zero();

			let allocation_json = &include_bytes!("../../../../resources/acala-allocation-ACA.json")[..];
			let initial_allocation: Vec<(AccountId, Balance)> = serde_json::from_slice(allocation_json).unwrap();
			let initial_authorities: Vec<(AccountId, AuraId)> = vec![
				(
					// 24j2ECgfuGHw2bv2YHLoFz88eKr39QAczGTz23bNLZKHEXdt
					hex!["aa66ae1c82621f3439a821974bfd285885ed2a513fc7ed660aa10dcf50161c7a"].into(),
					hex!["9ee6d04b7ae198f77cd4f4ed53ae2ce65ba978b9e140c67a52242b7b0c3ca425"].unchecked_into(),
				),
				(
					// 211oiNyWbThWJmuFSVJnGwdq4kPiYoMQ3fUKDHuHJnRxEymL
					hex!["0642caac4bb7be8367c277371825e1314be4ec99d9a0d0e2ed12289693009a6f"].into(),
					hex!["94231e6fe4b7868794b2c926e4e44c51a9944457559fd927ee078d465ef3bf1f"].unchecked_into(),
				),
				(
					// 21vkHrN6nQnZt5a3YWExxkAwMPepKyXHHDowG22fjxGbBLai
					hex!["2ea346904b62daeb65e158f15a7b4f74fa162b0e95a30dc9b6187f245f16bd0a"].into(),
					hex!["d2bc5f639405b8d36ebe2fc5700f17f65ee99386566d492a0882c2bf5ab28e10"].unchecked_into(),
				),
				(
					// 25j9RvPux27vBAk5qa919rf8BnihvMWPjr3gZLP3zT2HTWDa
					hex!["d6bb2868fa5a24d6776bc039a1689c9f1a9762f29266cc0519541a659abd5f76"].into(),
					hex!["30c13525850f92a53901c1d046f11a4a8859afa28051d44003617d1fb935d655"].unchecked_into(),
				),
			];

			let general_councils: Vec<AccountId> = vec![
				// 23RDJ7SyVgpKqC6M9ad8wvbBsbSr3R4Xqr5NQAKEhWPHbLbs
				hex!["7095491dc941e21b9269fe67b322311df5daafd75f0bf8868afd8fa828b06329"].into(),
				// 263KsUutx8qhRmG7hq6fEaSKE3fdi3KeeEKafkAMJ1cg1AYc
				hex!["e498b8bed2069371dc5ece389d7d60fe34a91fe4936f7f8eb8a84cd3e8dae34c"].into(),
				// 26VNG6LyuRag3xfuck7eoAjKk4ZLg9GeN6LDjxMw4ib3E8yg
				hex!["f87525a8a29cc3a1c56fb231a165d5fd38c42459f38c638c3a1d0f29061c101a"].into(),
				// 258WnzxhgwXuDL7w3Hag8TMCqb79dAUvRrMJd9kqJ9CzDf7v
				hex!["bc517c01c4b663efdfea3dd9ab71bdc3ea607e8a35ba3d1872e5b0942821cd2f"].into(),
				// 249QskFMEcb5WcgHF7BH5MesVGHq3imsUACq2RPgtBBdCPMa
				hex!["90c492f38270b5512370886c392ff6ec7624b14185b4b610b30248a28c94c953"].into(),
				// 26SUM8AN5MKefKCFiPDapUcQwHNfNzWYMyfUSVcqiFV2JYWc
				hex!["f63fe694d0c8a0703fc45362efc2852c8b8c9c4061b5f0cf9bd0329a984fc95d"].into(),
			];

			// 26Jo633eujX7UwGDp9tTwTuSqTq5thopn2QUKoFQhM4gvCZp
			let root_key: AccountId = hex!["f065057e73a3ffceff273f4555a0ea3d731ec8ef4d79954473b4ffda046d836d"].into();

			let unique_allocation_accounts = initial_allocation
				.iter()
				.map(|(account_id, amount)| {
					assert!(*amount >= existential_deposit, "allocation amount must gte ED");
					total_allocated = total_allocated
						.checked_add(*amount)
						.expect("shouldn't overflow when building genesis");

					account_id
				})
				.cloned()
				.collect::<std::collections::BTreeSet<_>>();
			assert!(
				unique_allocation_accounts.len() == initial_allocation.len(),
				"duplicate allocation accounts in genesis."
			);
			assert_eq!(
				total_allocated,
				1_000_000_000 * dollar(ACA), // 1 billion ACA
				"total allocation must be equal to 1 billion ACA"
			);

			let vesting_list_json = &include_bytes!("../../../../resources/acala-vesting-ACA.json")[..];
			let vesting_list: Vec<(AccountId, BlockNumber, BlockNumber, u32, Balance)> =
				serde_json::from_slice(vesting_list_json).unwrap();

			// ensure no duplicates exist.
			let unique_vesting_accounts = vesting_list
				.iter()
				.map(|(x, _, _, _, _)| x)
				.cloned()
				.collect::<std::collections::BTreeSet<_>>();
			assert!(
				unique_vesting_accounts.len() == vesting_list.len(),
				"duplicate vesting accounts in genesis."
			);

			acala_genesis(
				wasm_binary,
				initial_authorities,
				root_key,
				initial_allocation,
				vesting_list,
				general_councils,
			)
		},
		vec![
			"/dns/acala-rpc-0.aca-api.network/tcp/30333/p2p/12D3KooWASu892sCwPezdcqRbmS7HVYJcAfeMKQdewiRywYLeKL9"
				.parse()
				.unwrap(),
		],
		TelemetryEndpoints::new(vec![(TELEMETRY_URL.into(), 0)]).ok(),
		Some("acala"),
		None,
		Some(acala_properties()),
		Extensions {
			relay_chain: "polkadot".into(),
			para_id: PARA_ID,
			bad_blocks: None,
		},
	))
}

pub fn acala_dev_config() -> Result<ChainSpec, String> {
	let wasm_binary = acala_runtime::WASM_BINARY.unwrap_or_default();

	Ok(ChainSpec::from_genesis(
		"Acala Dev",
		"acala-dev",
		ChainType::Development,
		move || {
			acala_genesis(
				wasm_binary,
				// Initial PoA authorities
				vec![get_parachain_authority_keys_from_seed("Alice")],
				// Sudo account
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				vec![
					(get_account_id_from_seed::<sr25519::Public>("Alice"), 1000 * dollar(ACA)),
					(get_account_id_from_seed::<sr25519::Public>("Bob"), 1000 * dollar(ACA)),
					(
						get_account_id_from_seed::<sr25519::Public>("Charlie"),
						1000 * dollar(ACA),
					),
				],
				vec![],
				vec![get_account_id_from_seed::<sr25519::Public>("Alice")],
			)
		},
		vec![],
		None,
		None,
		None,
		Some(acala_properties()),
		Extensions {
			relay_chain: "rococo-local".into(),
			para_id: PARA_ID,
			bad_blocks: None,
		},
	))
}

fn acala_genesis(
	wasm_binary: &[u8],
	initial_authorities: Vec<(AccountId, AuraId)>,
	root_key: AccountId,
	initial_allocation: Vec<(AccountId, Balance)>,
	vesting_list: Vec<(AccountId, BlockNumber, BlockNumber, u32, Balance)>,
	general_councils: Vec<AccountId>,
) -> acala_runtime::GenesisConfig {
	acala_runtime::GenesisConfig {
		system: SystemConfig {
			// Add Wasm runtime to storage.
			code: wasm_binary.to_vec(),
		},
		balances: BalancesConfig {
			balances: initial_allocation,
		},
		sudo: SudoConfig { key: Some(root_key) },
		general_council: Default::default(),
		general_council_membership: GeneralCouncilMembershipConfig {
			members: general_councils,
			phantom: Default::default(),
		},
		financial_council: Default::default(),
		financial_council_membership: FinancialCouncilMembershipConfig {
			members: vec![],
			phantom: Default::default(),
		},
		homa_council: Default::default(),
		homa_council_membership: HomaCouncilMembershipConfig {
			members: vec![],
			phantom: Default::default(),
		},
		technical_committee: Default::default(),
		technical_committee_membership: TechnicalCommitteeMembershipConfig {
			members: vec![],
			phantom: Default::default(),
		},
		operator_membership_acala: OperatorMembershipAcalaConfig {
			members: vec![],
			phantom: Default::default(),
		},
		democracy: Default::default(),
		treasury: Default::default(),
		tokens: TokensConfig { balances: vec![] },
		vesting: VestingConfig { vesting: vesting_list },
		cdp_treasury: CdpTreasuryConfig {
			expected_collateral_auction_size: vec![],
		},
		cdp_engine: CdpEngineConfig {
			collaterals_params: vec![],
			global_interest_rate_per_sec: Default::default(),
		},
		evm: Default::default(),
		dex: DexConfig {
			initial_listing_trading_pairs: vec![],
			initial_enabled_trading_pairs: vec![],
			initial_added_liquidity_pools: vec![],
		},
		parachain_info: ParachainInfoConfig {
			parachain_id: PARA_ID.into(),
		},
		orml_nft: OrmlNFTConfig { tokens: vec![] },
		collator_selection: CollatorSelectionConfig {
			invulnerables: initial_authorities.iter().cloned().map(|(acc, _)| acc).collect(),
			candidacy_bond: Zero::zero(),
			..Default::default()
		},
		session: SessionConfig {
			keys: initial_authorities
				.iter()
				.cloned()
				.map(|(acc, aura)| {
					(
						acc.clone(),          // account id
						acc,                  // validator id
						SessionKeys { aura }, // session keys
					)
				})
				.collect(),
		},
		session_manager: SessionManagerConfig {
			session_duration: SessionDuration::get(),
		},
		// no need to pass anything to aura, in fact it will panic if we do. Session will take care
		// of this.
		aura: Default::default(),
		aura_ext: Default::default(),
		parachain_system: Default::default(),
		polkadot_xcm: PolkadotXcmConfig {
			safe_xcm_version: Some(2),
		},
	}
}
