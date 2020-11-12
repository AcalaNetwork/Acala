//! The precompiles for EVM, includes standard Ethereum precompiles, and more:
//! - MultiCurrency at address `H160::from_low_u64_be(1024)`.

use module_evm::{
	precompiles::{Precompile, Precompiles},
	AddressMapping, ExitError, ExitSucceed,
};
use primitives::PRECOMPILE_ADDRESS_START;
use sp_core::H160;
use sp_std::{marker::PhantomData, prelude::*};

pub mod multicurrency;
pub mod nft;

pub type EthereumPrecompiles = (
	module_evm::precompiles::ECRecover,
	module_evm::precompiles::Sha256,
	module_evm::precompiles::Ripemd160,
	module_evm::precompiles::Identity,
);

pub struct AllPrecompiles<MultiCurrencyPrecompile, NFTPrecompile>(
	PhantomData<(MultiCurrencyPrecompile, NFTPrecompile)>,
);

impl<MultiCurrencyPrecompile, NFTPrecompile> Precompiles for AllPrecompiles<MultiCurrencyPrecompile, NFTPrecompile>
where
	MultiCurrencyPrecompile: Precompile,
	NFTPrecompile: Precompile,
{
	#[allow(clippy::type_complexity)]
	fn execute(
		address: H160,
		input: &[u8],
		target_gas: Option<usize>,
	) -> Option<core::result::Result<(ExitSucceed, Vec<u8>, usize), ExitError>> {
		EthereumPrecompiles::execute(address, input, target_gas).or_else(|| {
			if address == H160::from_low_u64_be(PRECOMPILE_ADDRESS_START) {
				Some(MultiCurrencyPrecompile::execute(input, target_gas))
			} else if address == H160::from_low_u64_be(PRECOMPILE_ADDRESS_START + 1) {
				Some(NFTPrecompile::execute(input, target_gas))
			} else {
				None
			}
		})
	}
}

pub fn account_id_from_slice<AccountId, AccountIdConverter: AddressMapping<AccountId>>(src: &[u8]) -> AccountId {
	let mut address = [0u8; 20];
	address[..].copy_from_slice(src);
	AccountIdConverter::into_account_id(address.into())
}
