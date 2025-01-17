use cosmwasm_std::{coin, coins, Addr, BlockInfo, Coin, Empty, Event, Timestamp, Order};
use cw_multi_test::{next_block, AppResponse};

use crate::multitest::suite::TestingSuite;
use proposal::error::ContractError;
use proposal::proposal::state::ProposalStatus;
use proposal::msg::{ProposalBy, ProposalsResponse};

const INITIAL_BALANCE: u128 = 1_000_000;

#[test]
fn test_proposal_creation() {
    let mut suite = TestingSuite::default_with_balances(
        vec![coin(INITIAL_BALANCE, "uom")]
        );

    let admin = &suite.admin();
    let proposer = suite.senders[1].clone();
    let receiver = suite.senders[2].clone();

    // Test creating proposal with insufficient funds
    suite
    .instantiate_proposal_contract(Some(admin.to_string()))
    .create_proposal(
        &proposer,
        Some("Title".to_string()),
        Some("Speech".to_string()),
        receiver.to_string(),
        vec![],
        &[],
        |r| assert!(matches!(r, Err(_))),
    );

    // Test successful proposal creation
    suite.create_proposal(
        &proposer,
        Some("Title".to_string()),
        Some("Speech".to_string()),
        receiver.to_string(),
        vec![],
        &[coin(100, "uom")],
        |r: Result<AppResponse, anyhow::Error>| assert!(r.is_ok()),
    );

    // Verify proposal details
    suite.query_proposal(0, |r| {
        let proposal = r.unwrap();
        assert_eq!(proposal.proposer, proposer);
        assert_eq!(proposal.receiver, receiver);
        assert_eq!(proposal.status, ProposalStatus::Pending);
    });
}

#[test]
fn test_cancel_proposal() {
    let mut suite = TestingSuite::default_with_balances(
        vec![coin(INITIAL_BALANCE, "uom")]
        );

    let admin = &suite.admin();
    let proposer = suite.senders[1].clone();
    let receiver = suite.senders[2].clone();

    // Create proposal
    suite
    .instantiate_proposal_contract(Some(admin.to_string()))
    .create_proposal(
        &proposer,
        None,
        None,
        receiver.to_string(),
        vec![],
        &[coin(100, "uom")],
        |r: Result<AppResponse, anyhow::Error>| assert!(r.is_ok()),
    );

    // Test canceling by non-proposer
    suite.cancel_proposal(&receiver, 0, |r: Result<AppResponse, anyhow::Error>| {
        assert!(matches!(
            r.unwrap_err().downcast().unwrap(),
            ContractError::Unauthorized
        ))
    });

    // Test successful cancellation
    suite.cancel_proposal(&proposer, 0, |r: Result<AppResponse, anyhow::Error>| assert!(r.is_ok()));

    // Verify proposal was deleted
    suite.query_proposal(0, |r: Result<proposal::proposal::state::Proposal, cosmwasm_std::StdError>| assert!(r.is_err()));
}

#[test]
fn test_proposal_response() {
    let mut suite = TestingSuite::default_with_balances(
        vec![coin(INITIAL_BALANCE, "uom")]
        );

    let admin = &suite.admin();
    let proposer = suite.senders[1].clone();
    let receiver = suite.senders[2].clone();
    let gift = vec![coin(500, "uom")];

    // Create proposal with gift
    suite
    .instantiate_proposal_contract(Some(admin.to_string()))
    .create_proposal(
        &proposer,
        None,
        None,
        receiver.to_string(),
        gift.clone(),
        &[coin(600, "uom")], // 100 for fee + 500 for gift
        |r: Result<AppResponse, anyhow::Error>| assert!(r.is_ok()),
    );

    // Test saying yes by non-receiver
    suite.say_yes(&proposer, 0, None, |r: Result<AppResponse, anyhow::Error>| {
        assert!(matches!(
            r.unwrap_err().downcast().unwrap(),
            ContractError::Unauthorized
        ))
    });

    // Test successful yes response
    suite.say_yes(
        &receiver,
        0,
        Some("I accept!".to_string()),
        |r: Result<AppResponse, anyhow::Error>| assert!(r.is_ok()),
    );

    // Verify proposal status and gift transfer
    suite.query_proposal(0, |r: Result<proposal::proposal::state::Proposal, cosmwasm_std::StdError>| {
        let proposal = r.unwrap();
        assert_eq!(proposal.status, ProposalStatus::Yes);
        assert_eq!(proposal.reply, Some("I accept!".to_string()));
    });
}

#[test]
fn test_query_proposals() {
    let mut suite = TestingSuite::default_with_balances(
        vec![coin(INITIAL_BALANCE, "uom")]
        );

    let admin = &suite.admin();
    let proposer = suite.senders[1].clone();
    let receiver = suite.senders[2].clone();

    suite
    .instantiate_proposal_contract(Some(admin.to_string()));

    // Create multiple proposals
    for i in 0..3 {
        suite.create_proposal(
            &proposer,
            Some(format!("Title {}", i)),
            None,
            receiver.to_string(),
            vec![],
            &[coin(100, "uom")],
            |r: Result<AppResponse, anyhow::Error>| assert!(r.is_ok()),
        );
    }

    // Test query by proposer
    suite.query_proposals(
        None,
        Some(ProposalBy::Proposer(proposer.to_string())),
        None,
        Some(Order::Ascending),
        |r: Result<ProposalsResponse, cosmwasm_std::StdError>| {
            let proposals = r.unwrap().proposals;
            assert_eq!(proposals.len(), 3);
            assert!(proposals.iter().all(|p| p.proposer == proposer));
        },
    );

    // Test query by receiver
    suite.query_proposals(
        None,
        Some(ProposalBy::Receiver(receiver.to_string())),
        Some(ProposalStatus::Pending),
        None,
        |r: Result<ProposalsResponse, cosmwasm_std::StdError>| {
            let proposals = r.unwrap().proposals;
            assert_eq!(proposals.len(), 3);
            assert!(proposals.iter().all(|p| p.receiver == receiver));
        },
    );
}

#[test]
fn test_update_config() {
    let mut suite = TestingSuite::default_with_balances(
        vec![coin(INITIAL_BALANCE, "uom")]
        );

    let admin = suite.admin();
    let non_admin = suite.senders[1].clone();

    // Test update by non-admin
    suite
    .instantiate_proposal_contract(Some(admin.to_string()))
    .update_config(
        &non_admin,
        Some(coin(200, "uom")),
        |r: Result<AppResponse, anyhow::Error>| assert!(r.is_err()),
    );

    // Test successful update
    suite.update_config(
        &admin,
        Some(coin(200, "uom")),
        |r: Result<AppResponse, anyhow::Error>| assert!(r.is_ok()),
    );

    // Verify config update
    suite.query_config(|r: Result<proposal::proposal::state::Config, cosmwasm_std::StdError>| {
        let config = r.unwrap();
        assert_eq!(config.successful_proposal_fee, coin(200, "uom"));
    });
}