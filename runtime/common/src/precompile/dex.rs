use super::input::{Input, InputT};
use frame_support::debug;
use module_evm::{Context, ExitError, ExitSucceed, Precompile};
use module_support::DEXManager;
use primitives::{evm::AddressMapping as AddressMappingT, Balance, CurrencyId};
use sp_core::U256;
use sp_std::{convert::TryFrom, fmt::Debug, marker::PhantomData, prelude::*, result};

/// The `DEX` impl precompile.
///
///
/// `input` data starts with `action`.
///
/// Actions:
/// - Get liquidity. Rest `input` bytes: `currency_id_a`, `currency_id_b`.
/// - Swap with exact supply. Rest `input` bytes: `who`, `currency_id_a`,
///   `currency_id_b`, `supply_amount`, `min_target_amount`.
pub struct DexPrecompile<AccountId, AddressMapping, Dex>(PhantomData<(AccountId, AddressMapping, Dex)>);

enum Action {
	GetLiquidityPool,
	SwapWithExactSupply,
}

impl TryFrom<u8> for Action {
	type Error = ();

	fn try_from(value: u8) -> Result<Self, Self::Error> {
		match value {
			0 => Ok(Action::GetLiquidityPool),
			1 => Ok(Action::SwapWithExactSupply),
			_ => Err(()),
		}
	}
}

impl<AccountId, AddressMapping, Dex> Precompile for DexPrecompile<AccountId, AddressMapping, Dex>
where
	AccountId: Debug + Clone,
	AddressMapping: AddressMappingT<AccountId>,
	Dex: DEXManager<AccountId, CurrencyId, Balance>,
{
	fn execute(
		input: &[u8],
		_target_gas: Option<u64>,
		_context: &Context,
	) -> result::Result<(ExitSucceed, Vec<u8>, u64), ExitError> {
		//TODO: evaluate cost

		debug::debug!(target: "evm", "input: {:?}", input);

		let input = Input::<Action, AccountId, AddressMapping>::new(input);

		let action = input.action()?;

		match action {
			Action::GetLiquidityPool => {
				let currency_id_a = input.currency_id_at(1)?;
				let currency_id_b = input.currency_id_at(2)?;

				let (balance_a, balance_b) = Dex::get_liquidity_pool(currency_id_a, currency_id_b);

				// output
				let mut be_bytes = [0u8; 64];
				U256::from(balance_a).to_big_endian(&mut be_bytes[..32]);
				U256::from(balance_b).to_big_endian(&mut be_bytes[32..64]);

				Ok((ExitSucceed::Returned, be_bytes.to_vec(), 0))
			}
			Action::SwapWithExactSupply => {
				let who = input.account_id_at(1)?;
				let currency_id_a = input.currency_id_at(2)?;
				let currency_id_b = input.currency_id_at(3)?;
				let supply_amount = input.balance_at(4)?;
				let min_target_amount = input.balance_at(5)?;

				let value = Dex::swap_with_exact_supply(
					&who,
					&[currency_id_a, currency_id_b],
					supply_amount,
					min_target_amount,
					None,
				)
				.map_err(|e| {
					let err_msg: &str = e.into();
					ExitError::Other(err_msg.into())
				})?;

				// output
				let mut be_bytes = [0u8; 32];
				U256::from(value).to_big_endian(&mut be_bytes[..32]);

				Ok((ExitSucceed::Returned, be_bytes.to_vec(), 0))
			}
		}
	}
}
