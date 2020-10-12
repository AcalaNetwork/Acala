pub type ChainSpec = sc_service::GenericChainSpec<acala_runtime::GenesisConfig, crate::chain_spec::Extensions>;

pub fn acala_config() -> Result<ChainSpec, String> {
	Err("Not available".into())
}

pub fn latest_acala_config() -> Result<ChainSpec, String> {
	Err("Not available".into())
}
