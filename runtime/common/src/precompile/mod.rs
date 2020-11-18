//! The precompiles for EVM, includes standard Ethereum precompiles, and more:
//! - MultiCurrency at address `H160::from_low_u64_be(1024)`.

use module_evm::{
	precompiles::{Precompile, Precompiles},
	AddressMapping, Context, ExitError, ExitSucceed,
};
use module_support::PrecompileCallerFilter as PrecompileCallerFilterT;
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

pub struct AllPrecompiles<PrecompileCallerFilter, MultiCurrencyPrecompile, NFTPrecompile>(
	PhantomData<(PrecompileCallerFilter, MultiCurrencyPrecompile, NFTPrecompile)>,
);

impl<PrecompileCallerFilter, MultiCurrencyPrecompile, NFTPrecompile> Precompiles
	for AllPrecompiles<PrecompileCallerFilter, MultiCurrencyPrecompile, NFTPrecompile>
where
	MultiCurrencyPrecompile: Precompile,
	NFTPrecompile: Precompile,
	PrecompileCallerFilter: PrecompileCallerFilterT,
{
	#[allow(clippy::type_complexity)]
	fn execute(
		address: H160,
		input: &[u8],
		target_gas: Option<usize>,
		context: &Context,
	) -> Option<core::result::Result<(ExitSucceed, Vec<u8>, usize), ExitError>> {
		EthereumPrecompiles::execute(address, input, target_gas, context).or_else(|| {
			if !PrecompileCallerFilter::is_allowed(context.caller) {
				return Some(Err(ExitError::Other("no permission".into())));
			}

			if address == H160::from_low_u64_be(PRECOMPILE_ADDRESS_START) {
				Some(MultiCurrencyPrecompile::execute(input, target_gas, context))
			} else if address == H160::from_low_u64_be(PRECOMPILE_ADDRESS_START + 1) {
				Some(NFTPrecompile::execute(input, target_gas, context))
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
