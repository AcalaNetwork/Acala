#![cfg_attr(not(feature = "std"), no_std)]

pub trait Trait: system::Trait + auction_manager::Trait {}
