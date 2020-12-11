//! The precompiles for EVM, includes standard Ethereum precompiles, and more:
//! - MultiCurrency at address `H160::from_low_u64_be(1024)`.

use crate::is_acala_precompile;
use frame_support::debug;
use module_evm::{
	precompiles::{Precompile, Precompiles},
	Context, ExitError, ExitSucceed,
};
use module_support::PrecompileCallerFilter as PrecompileCallerFilterT;
use primitives::PRECOMPILE_ADDRESS_START;
use sp_core::H160;
use sp_std::{marker::PhantomData, prelude::*};

pub mod input;
pub mod multicurrency;
pub mod nft;

pub use multicurrency::MultiCurrencyPrecompile;
pub use nft::NFTPrecompile;

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
			if is_acala_precompile(address) && !PrecompileCallerFilter::is_allowed(context.caller) {
				debug::debug!(target: "evm", "Precompile no permission");
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

#[cfg(test)]
mod tests {
	use super::*;
	use primitives::PREDEPLOY_ADDRESS_START;

	pub struct DummyPrecompile;
	impl Precompile for DummyPrecompile {
		fn execute(
			_input: &[u8],
			_target_gas: Option<usize>,
			_context: &Context,
		) -> core::result::Result<(ExitSucceed, Vec<u8>, usize), ExitError> {
			Ok((ExitSucceed::Stopped, vec![], 0))
		}
	}

	pub type WithSystemContractFilter = AllPrecompiles<crate::SystemContractsFilter, DummyPrecompile, DummyPrecompile>;

	#[test]
	fn precompile_filter_works_on_acala_precompiles() {
		let precompile = H160::from_low_u64_be(PRECOMPILE_ADDRESS_START);

		let mut non_system = [0u8; 20];
		non_system[0] = 1;

		let non_system_caller_context = Context {
			address: precompile,
			caller: non_system.into(),
			apparent_value: 0.into(),
		};
		assert_eq!(
			WithSystemContractFilter::execute(precompile, &[0u8; 1], None, &non_system_caller_context),
			Some(Err(ExitError::Other("no permission".into()))),
		);
	}

	#[test]
	fn precompile_filter_does_not_work_on_system_contracts() {
		let system = H160::from_low_u64_be(PREDEPLOY_ADDRESS_START);

		let mut non_system = [0u8; 20];
		non_system[0] = 1;

		let non_system_caller_context = Context {
			address: system,
			caller: non_system.into(),
			apparent_value: 0.into(),
		};
		assert!(
			WithSystemContractFilter::execute(non_system.into(), &[0u8; 1], None, &non_system_caller_context).is_none()
		);
	}

	#[test]
	fn precompile_filter_does_not_work_on_non_system_contracts() {
		let mut non_system = [0u8; 20];
		non_system[0] = 1;
		let mut another_non_system = [0u8; 20];
		another_non_system[0] = 2;

		let non_system_caller_context = Context {
			address: non_system.into(),
			caller: another_non_system.into(),
			apparent_value: 0.into(),
		};
		assert!(
			WithSystemContractFilter::execute(non_system.into(), &[0u8; 1], None, &non_system_caller_context).is_none()
		);
	}
}
