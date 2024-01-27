// This file is part of Acala.

// Copyright (C) 2020-2023 Acala Foundation.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Councils for Gov1 and Gov2

use super::*;

parameter_types! {
	pub const GeneralCouncilMotionDuration: BlockNumber = 3 * DAYS;
	pub const CouncilDefaultMaxProposals: u32 = 20;
	pub const CouncilDefaultMaxMembers: u32 = 30;
	pub MaxProposalWeight: Weight = Perbill::from_percent(50) * RuntimeBlockWeights::get().max_block;
}

impl pallet_collective::Config<GeneralCouncilInstance> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = GeneralCouncilMotionDuration;
	type MaxProposals = CouncilDefaultMaxProposals;
	type MaxMembers = CouncilDefaultMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type SetMembersOrigin = EnsureRoot<AccountId>;
	type WeightInfo = ();
	type MaxProposalWeight = MaxProposalWeight;
}

impl pallet_membership::Config<GeneralCouncilMembershipInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AddOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type RemoveOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type SwapOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type ResetOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type PrimeOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type MembershipInitialized = GeneralCouncil;
	type MembershipChanged = GeneralCouncil;
	type MaxMembers = CouncilDefaultMaxMembers;
	type WeightInfo = ();
}

parameter_types! {
	pub const FinancialCouncilMotionDuration: BlockNumber = 3 * DAYS;
}

impl pallet_collective::Config<FinancialCouncilInstance> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = FinancialCouncilMotionDuration;
	type MaxProposals = CouncilDefaultMaxProposals;
	type MaxMembers = CouncilDefaultMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type SetMembersOrigin = EnsureRoot<AccountId>;
	type WeightInfo = ();
	type MaxProposalWeight = MaxProposalWeight;
}

impl pallet_membership::Config<FinancialCouncilMembershipInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AddOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type RemoveOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type SwapOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type ResetOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type PrimeOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type MembershipInitialized = FinancialCouncil;
	type MembershipChanged = FinancialCouncil;
	type MaxMembers = CouncilDefaultMaxMembers;
	type WeightInfo = ();
}

parameter_types! {
	pub const HomaCouncilMotionDuration: BlockNumber = 3 * DAYS;
}

impl pallet_collective::Config<HomaCouncilInstance> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = HomaCouncilMotionDuration;
	type MaxProposals = CouncilDefaultMaxProposals;
	type MaxMembers = CouncilDefaultMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type SetMembersOrigin = EnsureRoot<AccountId>;
	type WeightInfo = ();
	type MaxProposalWeight = MaxProposalWeight;
}

impl pallet_membership::Config<HomaCouncilMembershipInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AddOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type RemoveOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type SwapOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type ResetOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type PrimeOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type MembershipInitialized = HomaCouncil;
	type MembershipChanged = HomaCouncil;
	type MaxMembers = CouncilDefaultMaxMembers;
	type WeightInfo = ();
}

parameter_types! {
	pub const TechnicalCommitteeMotionDuration: BlockNumber = 3 * DAYS;
}

impl pallet_collective::Config<TechnicalCommitteeInstance> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = TechnicalCommitteeMotionDuration;
	type MaxProposals = CouncilDefaultMaxProposals;
	type MaxMembers = CouncilDefaultMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type SetMembersOrigin = EnsureRoot<AccountId>;
	type WeightInfo = ();
	type MaxProposalWeight = MaxProposalWeight;
}

impl pallet_membership::Config<TechnicalCommitteeMembershipInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AddOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type RemoveOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type SwapOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type ResetOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type PrimeOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type MembershipInitialized = TechnicalCommittee;
	type MembershipChanged = TechnicalCommittee;
	type MaxMembers = CouncilDefaultMaxMembers;
	type WeightInfo = ();
}
