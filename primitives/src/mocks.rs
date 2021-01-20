use codec::Encode;
use sp_core::{crypto::AccountId32, H160};
use sp_io::hashing::blake2_256;

pub struct MockAddressMapping;

impl crate::evm::AddressMapping<AccountId32> for MockAddressMapping {
	fn get_account_id(address: &H160) -> AccountId32 {
		let mut data = [0u8; 32];
		data[0..4].copy_from_slice(b"evm:");
		data[4..24].copy_from_slice(&address[..]);
		AccountId32::from(data)
	}

	fn get_evm_address(account_id: &AccountId32) -> Option<H160> {
		let data: [u8; 32] = account_id.clone().into();
		if data.starts_with(b"evm:") {
			Some(H160::from_slice(&data[4..24]))
		} else {
			None
		}
	}

	fn get_default_evm_address(account_id: &AccountId32) -> H160 {
		H160::from_slice(account_id.as_ref())
	}

	fn get_or_create_evm_address(account_id: &AccountId32) -> H160 {
		Self::get_evm_address(account_id).unwrap_or({
			let payload = (b"evm:", account_id);
			H160::from_slice(&payload.using_encoded(blake2_256)[0..20])
		})
	}

	fn is_linked(account_id: &AccountId32, evm: &H160) -> bool {
		Self::get_or_create_evm_address(account_id) == *evm
	}
}
