//! Tests for the bonecoin wallet

use bonecoin_core::*;
use std::collections::*;
use utxo_wallet_assignment::Wallet;

/// Simple helper to initialize a wallet with just one account.
fn wallet_with_alice() -> Wallet {
    Wallet::new(vec![Address::Alice].into_iter())
}

/// Helper to create a simple and somewhat collision unlikely transaction to mark forks.
/// When your tests create forked blockchain, you have to be sure that you are not accidentally
/// creating the same chain twice. It is sometimes useful, therefore, to put this little marker tx on the new side of the fork.
///
/// You can see examples of using this function in the tests below.
fn marker_tx() -> Transaction {
    Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 123,
            owner: Address::Custom(123),
        }],
    }
}

#[test]
fn correct_genesis_values() {
    let wallet = wallet_with_alice();

    assert_eq!(wallet.best_height(), 0);
    assert_eq!(wallet.best_hash(), Block::genesis().id());
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(0));
    assert_eq!(wallet.net_worth(), 0);
    assert_eq!(wallet.all_coins_of(Address::Alice).unwrap().len(), 0);
}

#[test]
fn foreign_address_error() {
    let wallet = wallet_with_alice();

    assert_eq!(
        wallet.total_assets_of(Address::Bob),
        Err(WalletError::ForeignAddress)
    );
    assert_eq!(
        wallet.all_coins_of(Address::Bob),
        Err(WalletError::ForeignAddress)
    );
}

#[test]
fn sync_two_blocks() {
    // Build a mock node that has a simple two block chain
    let mut node = MockNode::new();
    let b1_id = node.add_block(Block::genesis().id(), vec![]);
    let b2_id = node.add_block_as_best(b1_id, vec![]);

    let mut wallet = wallet_with_alice();
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 2);
    assert_eq!(wallet.best_hash(), b2_id);
}

#[test]
fn short_reorg() {
    // Create node and wallet
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice();

    // Sync a chain to height 1
    let _old_b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    wallet.sync(&node);

    // Reorg to longer chain of length 2
    let b1_id = node.add_block(Block::genesis().id(), vec![marker_tx()]);
    let b2_id = node.add_block_as_best(b1_id, vec![]);
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 2);
    assert_eq!(wallet.best_hash(), b2_id);
}

//          B2 (discard)  -  B3 (discard)
//        /
//    G
//        \
//          C2            -  C3             -       C4          -        C5 (new wallet state)
#[test]
fn deep_reorg() {
    // Create node and wallet
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice();

    // Sync a chain to height 3
    let old_b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    let old_b2_id = node.add_block_as_best(old_b1_id, vec![]);
    let _old_b3_id = node.add_block_as_best(old_b2_id, vec![]);
    wallet.sync(&node);

    let b1_id = node.add_block(Block::genesis().id(), vec![marker_tx()]);
    let b2_id = node.add_block_as_best(b1_id, vec![]);
    let b3_id = node.add_block_as_best(b2_id, vec![]);
    let b4_id = node.add_block_as_best(b3_id, vec![]);
    let b5_id = node.add_block_as_best(b4_id, vec![]);
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 5);
    assert_eq!(wallet.best_hash(), b5_id);
}

//                      Old_B2 (discard)   -     Old_B3 (discard)
//                  /
//              G
//                  \   B2      (should reorg the chain here)
#[test]
fn reorg_to_shorter_chain() {
    // Create node and wallet
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice();

    // Sync a chain to height 3
    let old_b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    let old_b2_id = node.add_block_as_best(old_b1_id, vec![]);
    let _old_b3_id = node.add_block_as_best(old_b2_id, vec![]);
    wallet.sync(&node);

    // Reorg to shorter chain of length 2
    let b1_id = node.add_block(Block::genesis().id(), vec![marker_tx()]);
    let b2_id = node.add_block_as_best(b1_id, vec![]);
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 2);
    assert_eq!(wallet.best_hash(), b2_id);
}

#[test]
fn tracks_single_utxo() {
    // We have a single transaction that consumes some made up input
    // and creates a single output to alice.
    const COIN_VALUE: u64 = 100;
    let coin = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin.clone()],
    };
    let coin_id = tx.coin_id(1, 0);

    // Create a minimal chain to contain this transaction and sync it
    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), vec![tx]);

    let mut wallet = wallet_with_alice();
    wallet.sync(&node);

    // Check that the accounting is right
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(COIN_VALUE));
    assert_eq!(wallet.net_worth(), COIN_VALUE);
    assert_eq!(
        wallet.all_coins_of(Address::Alice),
        Ok(HashSet::from_iter([(coin_id, COIN_VALUE)]))
    );
    assert_eq!(wallet.coin_details(&coin_id), Ok(coin));
}

#[test]
fn consumes_own_utxo() {
    // All coins will be valued the same in this test
    const COIN_VALUE: u64 = 100;

    // We start by minting a coin to alice
    let coin = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let tx_mint = Transaction {
        inputs: vec![],
        outputs: vec![coin.clone()],
    };
    let coin_id = tx_mint.coin_id(1, 0);

    // Then we burn that coin
    let input = Input {
        coin_id,
        // The signature is invalid to save syntax.
        // The wallet doesn't check validity anyway.
        // This transaction is in a block, so the wallet syncs it.
        signature: Signature::Invalid,
    };
    let tx_burn = Transaction {
        inputs: vec![input],
        outputs: vec![],
    };

    // Apply this all to a blockchain and sync the wallet.
    // We apply in two separate blocks although that shouldn't be necessary.
    let mut node = MockNode::new();
    let b1_id = node.add_block_as_best(Block::genesis().id(), vec![tx_mint]);
    let _b2_id = node.add_block_as_best(b1_id, vec![tx_burn]);
    let mut wallet = wallet_with_alice();
    wallet.sync(&node);

    // Make sure the UTXO is consumed
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(0));
    assert_eq!(wallet.net_worth(), 0);
    assert_eq!(wallet.all_coins_of(Address::Alice), Ok(HashSet::new()));
    // Pedagogy: It is reasonable that the wallet could provide details about
    // the coin even after it was spent. But requiring that gives away the trick of
    // tracking spent coins so you can revert them later.
    assert_eq!(wallet.coin_details(&coin_id), Err(WalletError::UnknownCoin));
}

fn make_one_block_blockchain() -> (MockNode, Wallet) {
    // simple blockchain to test transaction creation

    // minting coins
    let coin_alice_1 = Coin {
        value: 100,
        owner: Address::Alice,
    };
    let coin_alice_2 = Coin {
        value: 15,
        owner: Address::Alice,
    };
    let coin_bob_1 = Coin {
        value: 120,
        owner: Address::Bob,
    };

    let tx_mint = Transaction {
        inputs: vec![],
        outputs: vec![
            coin_alice_1.clone(),
            coin_alice_2.clone(),
            coin_bob_1.clone(),
        ],
    };

    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), vec![tx_mint]);

    let mut wallet = Wallet::new(vec![Address::Alice, Address::Bob].into_iter());
    wallet.sync(&node);

    (node, wallet)
}

#[test]
fn blockchain_creation() {
    let (_node, wallet) = make_one_block_blockchain();

    // MODIFIED: commented this out
    // wallet.print_utxo();
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(100 + 15));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(120));
    assert_eq!(wallet.net_worth(), 100 + 15 + 120);
}

#[test]
fn empty_wallet_fails_transaction() {
    let wallet = Wallet::new(vec![].into_iter());
    let result = wallet.create_automatic_transaction(Address::Charlie, 10, 3);
    assert!(matches!(result, Err(WalletError::NoOwnedAddresses)));
}

#[test]
fn transaction_with_zero_value_fails() {
    let (_, wallet) = make_one_block_blockchain();

    // now test with manual
    let (coin_id, _) = wallet
        .all_coins_of(Address::Alice)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let result = wallet.create_manual_transaction(
        vec![coin_id],
        vec![Coin {
            value: 0,
            owner: Address::Eve,
        }],
    );
    assert_eq!(result, Err(WalletError::ZeroCoinValue));

    // now check a failing transaction to zero value outputs for both automatic and manual transactions
    let result = wallet.create_automatic_transaction(Address::Charlie, 0, 0);
    assert_eq!(result, Err(WalletError::ZeroCoinValue));
}

#[test]
fn process_new_block() {
    let (mut node, mut wallet) = make_one_block_blockchain();

    let result = wallet.create_automatic_transaction(Address::Charlie, 26, 2);
    let tx = result.unwrap();
    let b1_id = node.best_block_at_height(1).unwrap();
    node.add_block_as_best(b1_id, vec![tx]);
    wallet.sync(&node);

    // MODIFIED: commented this out
    // wallet.print_utxo();

    assert_eq!(wallet.net_worth(), (100 + 15 + 120 - 26 - 2));
}

#[test]
fn transaction_simple() {
    let (_, wallet) = make_one_block_blockchain();

    let result = wallet.create_automatic_transaction(Address::Charlie, 26, 2);
    assert!(result.is_ok());
}

#[test]
fn transaction_automatic_insufficient_funds() {
    let (_, wallet) = make_one_block_blockchain();

    // now check a failing transaction due to insufficient funds
    let result = wallet.create_automatic_transaction(Address::Charlie, wallet.net_worth() - 3, 4);
    assert_eq!(result, Err(WalletError::InsufficientFunds));
}

#[test]
fn sneak_in_no_inputs() {
    let (_, wallet) = make_one_block_blockchain();
    // now try to sneak in no inputs but get an output going
    let result = wallet.create_manual_transaction(
        vec![], // no inputs
        vec![Coin {
            value: 10,
            owner: Address::Charlie, // output to Bob
        }],
    );
    assert_eq!(result, Err(WalletError::ZeroInputs));
}

#[test]
fn sneak_in_non_owned_address() {
    let (_, wallet) = make_one_block_blockchain();

    // sneak in non-owned address
    let result = wallet.create_automatic_transaction(Address::Charlie, 26, 2);
    let tx = result.unwrap();
    let charlie_coin_id = tx.coin_id(1, 1);

    let result = wallet.create_manual_transaction(
        vec![charlie_coin_id],
        vec![Coin {
            value: 10,
            owner: Address::Eve,
        }],
    );
    assert_eq!(result, Err(WalletError::UnknownCoin));
}

#[test]
fn transaction_with_no_change_tx() {
    let (_, wallet) = make_one_block_blockchain();

    // try to create a transaction with balance exactly equal to output + burn, there won't be a change output
    let result = wallet.create_automatic_transaction(Address::Charlie, wallet.net_worth() - 3, 3);
    // MODIFIED: changed from 2 to 1, since burned coins should not be in the output
    assert!(result.unwrap().outputs.len() == 1);
}

#[test]
fn utxo_reog_simple() {
    let mut node = MockNode::new();
    let mut wallet = Wallet::new(vec![Address::Alice, Address::Bob].into_iter());

    let tx_1 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 27,
            owner: Address::Alice,
        }],
    };

    // Old chain
    let b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    let b2_id = node.add_block_as_best(b1_id, vec![]);
    let old_b3_id = node.add_block_as_best(b2_id, vec![tx_1.clone()]);

    wallet.sync(&node);
    // wallet.print_utxo();

    assert_eq!(wallet.best_height(), 3);
    assert_eq!(wallet.best_hash(), old_b3_id);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(27));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(0));

    // now reog to a new chain where alice's 27 token is dropped but bob received 13
    let tx_2 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 13,
            owner: Address::Bob,
        }],
    };

    let b3_id = node.add_block_as_best(b2_id, vec![tx_2]);
    let b4_id = node.add_block_as_best(b3_id, vec![]);

    wallet.sync(&node);
    // wallet.print_utxo();

    assert_eq!(wallet.best_height(), 4);
    assert_eq!(wallet.best_hash(), b4_id);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(0));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(13));
}

// add a test to send in a coin to an address, then reorg that tranaction away
// get coin details and see if you get error for the coin. you should

// Track UTXOs from two transactions in a single block
// Track UTXOs to multiple users

// Reorgs with UTXOs in the chain history

// Reorg performance tests to make sure they aren't just syncing from genesis each time.

// Memory performance test to make sure they aren't just keeping a snapshot of the entire UTXO set at every height.

// Create manual transaction
// ... with missing input - DONE
// ... with too much output - DONE
// ... with zero output value - DONE

// Create automatic transactions
// ... with too much output - DONE
// ... with zero change - DONE
