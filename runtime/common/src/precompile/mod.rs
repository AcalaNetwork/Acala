//! The precompiles for EVM, includes standard Ethereum precompiles, and more:
//! - MultiCurrency at address `H160::from_low_u64_be(1024)`.

use module_evm::{
	precompiles::{Precompile, Precompiles},
	ExitError, ExitSucceed,
};
use primitives::PRECOMPILE_ADDRESS_START;
use sp_core::H160;
use sp_std::{marker::PhantomData, prelude::*};

pub mod multicurrency;

pub type EthereumPrecompiles = (
	module_evm::precompiles::ECRecover,
	module_evm::precompiles::Sha256,
	module_evm::precompiles::Ripemd160,
	module_evm::precompiles::Identity,
);

pub struct AllPrecompiles<MultiCurrencyPrecompile>(PhantomData<MultiCurrencyPrecompile>);

impl<MultiCurrencyPrecompile: Precompile> Precompiles for AllPrecompiles<MultiCurrencyPrecompile> {
	#[allow(clippy::type_complexity)]
	fn execute(
		address: H160,
		input: &[u8],
		target_gas: Option<usize>,
	) -> Option<core::result::Result<(ExitSucceed, Vec<u8>, usize), ExitError>> {
		EthereumPrecompiles::execute(address, input, target_gas).or_else(|| {
			if address == H160::from_low_u64_be(PRECOMPILE_ADDRESS_START) {
				Some(MultiCurrencyPrecompile::execute(input, target_gas))
			} else {
				None
			}
		})
	}
}
