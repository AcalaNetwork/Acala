use codec::FullCodec;

use frame_support::debug;
use pallet_evm::{AddressMapping, ExitError, ExitSucceed, Precompile};
use sp_core::U256;
use sp_runtime::traits::MaybeSerializeDeserialize;
use sp_std::{convert::TryInto, fmt::Debug, marker::PhantomData, prelude::*, result};

use orml_traits::NFT;

use primitives::NFTBalance;
use super::account_id_from_slice;

/// The `NFT` impl precompile.
///
/// All `input` data start with `action_byte`;
///
/// The first byte of `input` indicates action.
/// -. `0`: Query balance. Rest: `account_id`.
/// -. `1`: Query owner. Rest `class_id ++ token_id`.
/// -. `2`: Transfer. Rest: `from ++ to ++ class_id ++ token_id`.
pub struct NFTPrecompile<AccountId, AccountIdConverter, NFTImpl>(PhantomData<(AccountId, AccountIdConverter, NFTImpl)>);

enum Action {
	QueryBalance,
	QueryOwner,
	Transfer,
	Unknown,
}

impl From<u8> for Action {
	fn from(a: u8) -> Self {
		match a {
			0 => Action::QueryBalance,
			1 => Action::QueryOwner,
			2 => Action::Transfer,
			_ => Action::Unknown,
		}
	}
}

impl<AccountId, AccountIdConverter, NFTImpl> Precompile for NFTPrecompile<AccountId, AccountIdConverter, NFTImpl>
where
	AccountId: Debug + Clone,
	AccountIdConverter: AddressMapping<AccountId>,
	NFTImpl: NFT<AccountId>,
{
	fn execute(input: &[u8], _target_gas: Option<usize>) -> result::Result<(ExitSucceed, Vec<u8>, usize), ExitError> {
		debug::info!("----------------------------------------------------------------");
		debug::info!(">>> input: {:?}", input);

		if input.len() < 2 {
			return Err(ExitError::Other("invalid input"));
		}
		let action: Action = input[0].into();
		debug::info!("action: {:?}", action);

		match action {
			Action::QueryBalance => {
				// 32 * 2
				if input.len() < 64 {
					return Err(ExitError::Other("invalid input"));
				}

				let who = account_id_from_slice::<_, AccountIdConverter>(&input[32..52]);
				let balance = vec_u8_from_balance(NFTImpl::balance(&who));

				debug::info!(">>> account id: {:?}", who);
				debug::info!(">>> balance: {:?}", balance);

				Ok((ExitSucceed::Returned, balance, 0))
			},
			Action::QueryOwner => {
				// 32 * 2
				if input.len() < 64 {
					return Err(ExitError::Other("invalid input"));
				}

				let class_id = u64_from_slice(&input[32..48]);
				let token_id = u64_from_slice(&input[48..64]);

				let owner = NFTImpl::owner(class_id, token_id);
			}
		}
	}
}

fn vec_u8_from_balance(b: NFTBalance) -> Vec<u8> {
	let mut be_bytes = [0u8; 32];
	U256::from(b).to_big_endian(&mut be_bytes[..]);
	be_bytes.to_vec()
}

fn u64_from_slice(src: &[u8]) -> u64 {
	let mut int = [0u8; 8];
	int[..].copy_from_slice(src);
	u64::from_be_bytes(int)
}
