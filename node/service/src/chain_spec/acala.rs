// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
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
use sp_std::collections::btree_map::BTreeMap;

use crate::chain_spec::{get_account_id_from_seed, get_parachain_authority_keys_from_seed, Extensions, TELEMETRY_URL};

use acala_runtime::{
	dollar, get_all_module_accounts, Balance, BalancesConfig, BlockNumber, CdpEngineConfig, CdpTreasuryConfig,
	CollatorSelectionConfig, DexConfig, FinancialCouncilMembershipConfig, GeneralCouncilMembershipConfig,
	HomaCouncilMembershipConfig, NativeTokenExistentialDeposit, OperatorMembershipAcalaConfig, OrmlNFTConfig,
	ParachainInfoConfig, PolkadotXcmConfig, SS58Prefix, SessionConfig, SessionDuration, SessionKeys,
	SessionManagerConfig, SudoConfig, SystemConfig, TechnicalCommitteeMembershipConfig, TokensConfig, VestingConfig,
	ACA, AUSD, DOT, LDOT,
};
use runtime_common::TokenInfo;

pub type ChainSpec = sc_service::GenericChainSpec<acala_runtime::GenesisConfig, Extensions>;

pub const PARA_ID: u32 = 2000;

pub fn acala_config() -> Result<ChainSpec, String> {
	ChainSpec::from_json_bytes(&include_bytes!("../../../../resources/acala-dist.json")[..])
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

			let airdrop_accounts_json = &include_bytes!("../../../../resources/mandala-airdrop-ACA.json")[..];
			let airdrop_accounts: Vec<(AccountId, Balance)> = serde_json::from_slice(airdrop_accounts_json).unwrap();
			let other_allocation_json = &include_bytes!("../../../../resources/acala-allocation-ACA.json")[..];
			let other_allocation: Vec<(AccountId, Balance)> = serde_json::from_slice(other_allocation_json).unwrap();

			// TODO: update
			let initial_authorities: Vec<(AccountId, AuraId)> = vec![
				(
					// qkFZUE2Dod2Y9LX8ZQzkvF5T2wE5hpBuPe9hT1gpP3drH1v
					hex!["6c47c55604029bd43ed443ddaad370d5f4c10fa439d22dddb8120a9615444b6b"].into(),
					hex!["36589a134ccdbeb45a3ac535cc2c8cd71ae45ffc3af86d4a020cc2e411a98875"].unchecked_into(),
				),
				(
					// pSCWXtDyPZsyfTQNbVkmubVRGyoSi9N2a6AxpWHWFsxLjXs
					hex!["3246d9cb076cd554f250fc03bf70988cbaa9cbb2c4b1b8e015dd97fd19405d43"].into(),
					hex!["3266d0febeacc5d111c9df7f2ced2f533e7732dda46b2b84f104be5d6e395b76"].unchecked_into(),
				),
				(
					// qZhHE2FJGGAJtvu9f21PPFVDxvcnm65ebezZBsAJjGFa4kn
					hex!["643aa70071341b904e6e5b4e41d6dfc02b4cfcdc4c9c7a66f41fc0e59c07e24c"].into(),
					hex!["60fdcbd860869ee9b1230731b82604e8cf63c6c66e69277b59e337f1f25af225"].unchecked_into(),
				),
			];

			// TODO: update
			let general_councils: Vec<AccountId> = vec![
				// ouJX1WJQ9s4RMukAx5zvMwPY2zJZ9Xr5euzRG97Ne6UTNG9
				hex!["1ab677fa2007fb1e8ac2f5f6d253d5a2bd9c2ed4e5d3c1565c5d84436f81325d"].into(),
				// qMJYLJEP2HTBFhxqTFAJz9RcsT9UQ3VW2tFHRBmyaxPdj1n
				hex!["5ac728d31a0046274f1c5bece1867555c6728c8e8219ff77bb7a8afef4ab8137"].into(),
				// qPnkT89PRdiCbBgvE6a6gLcFCqWC8F1UoCZUhFvjbBkXMXc
				hex!["5cac9c2837017a40f90cc15b292acdf1ee28ae03005dff8d13d32fdf7d2e237c"].into(),
				// sZCH1stvMnSuDK1EDpdNepMYcpZWoDt3yF3PnUENS21f2tA
				hex!["bc517c01c4b663efdfea3dd9ab71bdc3ea607e8a35ba3d1872e5b0942821cd2f"].into(),
				// ra6MmAYU2qdCVsMS3REKZ82CJ1EwMWq6H6Zo475xTzedctJ
				hex!["90c492f38270b5512370886c392ff6ec7624b14185b4b610b30248a28c94c953"].into(),
				// ts9q95ZJmaCMCPKuKTY4g5ZeK65GdFVz6ZDD8LEnYJ3jpbm
				hex!["f63fe694d0c8a0703fc45362efc2852c8b8c9c4061b5f0cf9bd0329a984fc95d"].into(),
			];

			// TODO: update
			// sWcq8FAQXPdXGSaxSTBKS614hCB8YutkVWWacBKG1GbGS23
			let root_key: AccountId = hex!["ba5a672d05b5db2ff433ee3dc24cf021e301bc9d44232046ce7bd45a9360fa50"].into();

			let initial_allocation = initial_authorities
				.iter()
				.map(|x| (x.0.clone(), existential_deposit))
				.chain(airdrop_accounts)
				.chain(other_allocation)
				.chain(
					get_all_module_accounts()
						.iter()
						.map(|x| (x.clone(), existential_deposit)), // add ED for module accounts
				)
				.fold(
					BTreeMap::<AccountId, Balance>::new(),
					|mut acc, (account_id, amount)| {
						// merge duplicated accounts
						if let Some(balance) = acc.get_mut(&account_id) {
							*balance = balance
								.checked_add(amount)
								.expect("balance cannot overflow when building genesis");
						} else {
							acc.insert(account_id.clone(), amount);
						}

						total_allocated = total_allocated
							.checked_add(amount)
							.expect("total insurance cannot overflow when building genesis");
						acc
					},
				)
				.into_iter()
				.collect::<Vec<(AccountId, Balance)>>();

			// check total allocated
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
		// TODO: update
		vec![
			"/dns/karura-rpc-0.aca-api.network/tcp/30333/p2p/12D3KooWDVQHcjsM5UkWKhfpxiNhWofmX5bvJd5Wn9qPFZk1C8t8"
				.parse()
				.unwrap(),
		],
		TelemetryEndpoints::new(vec![(TELEMETRY_URL.into(), 0)]).ok(),
		Some("acala"),
		Some(acala_properties()),
		Extensions {
			relay_chain: "polkadot".into(),
			para_id: PARA_ID,
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
		Some(acala_properties()),
		Extensions {
			relay_chain: "rococo-local".into(),
			para_id: PARA_ID,
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
			changes_trie_config: Default::default(),
		},
		balances: BalancesConfig {
			balances: initial_allocation,
		},
		sudo: SudoConfig { key: root_key },
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
