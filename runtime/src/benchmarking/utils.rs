use crate::{AccountId, Runtime};

use sp_runtime::traits::StaticLookup;

pub fn lookup_of_account(who: AccountId) -> <<Runtime as frame_system::Trait>::Lookup as StaticLookup>::Source {
	<Runtime as frame_system::Trait>::Lookup::unlookup(who)
}
