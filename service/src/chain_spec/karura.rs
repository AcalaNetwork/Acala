pub type ChainSpec = sc_service::GenericChainSpec<karura_runtime::GenesisConfig, crate::chain_spec::Extensions>;

pub fn karura_config() -> Result<ChainSpec, String> {
	Err("Not available".into())
}

pub fn latest_karura_config() -> Result<ChainSpec, String> {
	Err("Not available".into())
}
