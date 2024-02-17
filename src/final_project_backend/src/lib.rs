use candid::{CandidType, Decode, Deserialize, Encode};
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;

const MAX_VALUE_SIZE: u32 = 5000;

#[derive(Debug,CandidType,Deserialize)]
enum Choice {
    Approve,
    Reject,
    Pass,
}

#[derive(Debug,CandidType,Deserialize)]
enum VoteError {
    AlreadyVoted,
    ProposalIsNotActive,
    NoSuchProposal,
    AccessRejected,
    UpdateError
}

#[derive(Debug,CandidType,Deserialize)]
struct Proposal {
    description: String,
    approve: u32,
    reject: u32,
    pass: u32,
    is_active: bool,
    voted: Vec<candid::Principal>,
    owner: candid::Principal
}

#[derive(Debug,CandidType,Deserialize)]
struct CreateProposal {
    description: String,
    is_active: bool,
}

impl Storable for Proposal {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for Proposal {
    const MAX_SIZE: u32 = MAX_VALUE_SIZE;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static PROPOSAL_MAP: RefCell<StableBTreeMap<u64,Proposal,Memory>> = RefCell::new(StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0)))));

}

#[ic_cdk::query]
fn get_proposal(key: u64) -> Option<Proposal> {
    PROPOSAL_MAP.with(|p| p.borrow().get(&key))
}

#[ic_cdk::query]
fn get_proposal_count() -> u64 {
    PROPOSAL_MAP.with(|p| p.borrow().len())
}

#[ic_cdk::update]
fn create_proposal(key: u64, proposal: CreateProposal) -> Option<Proposal> {
    let value = Proposal {
        description: proposal.description,
        approve: 0u32,
        reject: 0u32,
        pass: 0u32,
        is_active: proposal.is_active,
        voted: vec![],
        owner: ic_cdk::caller()
    };

    PROPOSAL_MAP.with(|p| p.borrow_mut().insert(key, value))
}

#[ic_cdk::update]
fn edit_proposal(key: u64, proposal: CreateProposal) -> Result<(), VoteError> {
    PROPOSAL_MAP.with(|p|{
        let old_proposal_opt = p.borrow().get(&key);
        let old_proposal:Proposal;

        match old_proposal_opt {
            Some(value) => old_proposal = value,
            None => return Err(VoteError::NoSuchProposal),
        }

        if old_proposal.owner != ic_cdk::caller(){
            return Err(VoteError::AccessRejected);
        }

        let value = Proposal{
            description: proposal.description,
            approve: old_proposal.approve,
            reject: old_proposal.reject,
            pass: old_proposal.pass,
            is_active: proposal.is_active,
            voted: old_proposal.voted,
            owner: old_proposal.owner
        };

        let res = p.borrow_mut().insert(key, value);
        match res {
            Some(_) => Ok(()),
            None => Err(VoteError::UpdateError)
        }
    })
}

#[ic_cdk::update]
fn end_proposal(key: u64) -> Result<(), VoteError> {
    PROPOSAL_MAP.with(|p|{
        let old_proposal_opt = p.borrow().get(&key);
        let mut old_proposal:Proposal;

        match old_proposal_opt {
            Some(value) => old_proposal = value,
            None => return Err(VoteError::NoSuchProposal),
        }

        if old_proposal.owner != ic_cdk::caller(){
            return Err(VoteError::AccessRejected);
        }

        old_proposal.is_active = false;

        let res = p.borrow_mut().insert(key, old_proposal);
        match res {
            Some(_) => Ok(()),
            None => Err(VoteError::UpdateError)
        }
    })
}

#[ic_cdk::update]
fn vote(key: u64, choice: Choice) -> Result<(), VoteError> {
    PROPOSAL_MAP.with(|p|{
        let proposal_opt = p.borrow().get(&key);
        let mut proposal:Proposal;

        match proposal_opt {
            Some(value) => proposal = value,
            None => return Err(VoteError::NoSuchProposal),
        }
        let caller = ic_cdk::caller();
        if proposal.voted.contains(&caller){
            return Err(VoteError::AlreadyVoted);
        }else if proposal.is_active != true {
            return Err(VoteError::ProposalIsNotActive);
        }

        match choice {
            Choice::Approve => proposal.approve += 1,
            Choice::Pass => proposal.pass += 1,
            Choice::Reject => proposal.reject += 1,
        }

        proposal.voted.push(caller);
        let res = p.borrow_mut().insert(key, proposal);

        match res {
            Some(_) => Ok(()),
            None => Err(VoteError::UpdateError)
        }
    })
}
