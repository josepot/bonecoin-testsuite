//! Written by krayt78

//! Tests for the bonecoin wallet

use std::collections::*;
use bonecoin_core::*;
use utxo_wallet_assignment::Wallet;

/// Simple helper to initialize a wallet with just one account.
fn wallet_with_alice() -> Wallet {
    Wallet::new(vec![Address::Alice].into_iter())
}

fn wallet_with_alice_and_bob() -> Wallet {
    Wallet::new(vec![Address::Alice, Address::Bob].into_iter())
}

/// Helper to create a simple and somewhat collision unlikely transaction to mark forks.
/// When your tests create forked blockchain, you have have to be sure that you are not accidentally
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

    // Make sure the UTXO is cot is reasonable that the wallet could provide details about
    // the coin even after it was spent.nsumed
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(0));
    assert_eq!(wallet.net_worth(), 0);
    assert_eq!(wallet.all_coins_of(Address::Alice), Ok(HashSet::new()));
    // Pedagogy: It is reasonable that the wallet could provide details about
    // the coin even after it was spent. But requiring that gives away the trick of
    // tracking spent coins so you can revert them later.
    assert_eq!(wallet.coin_details(&coin_id), Err(WalletError::UnknownCoin));
}

// Track UTXOs from two transactions in a single block
#[test]
fn tracks_multiple_utxos() {
    // We have a single transaction that consumes some made up input
    // and creates a single output to alice.
    const COIN_VALUE: u64 = 100;
    let coin1 = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let coin2 = Coin {
        value: COIN_VALUE * 2,
        owner: Address::Alice,
    };
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin1.clone(), coin2.clone()],
    };
    let coin1_id = tx.coin_id(1, 0);
    let coin2_id = tx.coin_id(1, 1);

    // Create a minimal chain to contain this transaction and sync it
    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), vec![tx]);

    let mut wallet = wallet_with_alice();
    wallet.sync(&node);

    // Check that the accounting is right
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(COIN_VALUE * 3));
    assert_eq!(wallet.net_worth(), COIN_VALUE * 3);
    assert_eq!(
        wallet.all_coins_of(Address::Alice),
        Ok(HashSet::from_iter([(coin1_id, COIN_VALUE), (coin2_id, COIN_VALUE * 2)]))
    );
    assert_eq!(wallet.coin_details(&coin1_id), Ok(coin1));
    assert_eq!(wallet.coin_details(&coin2_id), Ok(coin2));
}   

// Track UTXOs to multiple users
#[test]
fn track_utxos_to_multiple_users() {
    // We have a single transaction that consumes some made up input
    // and creates a single output to alice.
    const COIN_VALUE: u64 = 100;
    let coin1 = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let coin2 = Coin {
        value: COIN_VALUE * 2,
        owner: Address::Bob,
    };
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin1.clone(), coin2.clone()],
    };
    let coin1_id = tx.coin_id(1, 0);
    let coin2_id = tx.coin_id(1, 1);

    // Create a minimal chain to contain this transaction and sync it
    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), vec![tx]);

    let mut wallet = wallet_with_alice_and_bob();
    wallet.sync(&node);

    // Check that the accounting is right
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(COIN_VALUE));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(COIN_VALUE * 2));
    assert_eq!(wallet.net_worth(), COIN_VALUE * 3);
    assert_eq!(
        wallet.all_coins_of(Address::Alice),
        Ok(HashSet::from_iter([(coin1_id, COIN_VALUE)]))
    );
    assert_eq!(
        wallet.all_coins_of(Address::Bob),
        Ok(HashSet::from_iter([(coin2_id, COIN_VALUE * 2)]))
    );
    assert_eq!(wallet.coin_details(&coin1_id), Ok(coin1));
    assert_eq!(wallet.coin_details(&coin2_id), Ok(coin2));
} 

// Create manual transaction
// ... with missing input
#[test]
fn check_manual_transaction_with_missing_input() {
    let wallet = wallet_with_alice();
    const COIN_VALUE: u64 = 100;
    let coin = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![],
    };

    assert_eq!(
        wallet.create_manual_transaction(vec![tx.coin_id(1, 0)], vec![coin]),
        Err(WalletError::UnknownCoin)
    );
} 

// ... with owner address to not be in the wallet
#[test]
fn check_manual_transaction_with_wrong_input_addresses() {
    const COIN_VALUE: u64 = 100;
    let coin = Coin {
        value: COIN_VALUE,
        owner: Address::Bob,
    };
    
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin.clone()],
    };

    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), vec![tx.clone()]);

    let mut wallet = wallet_with_alice();
    wallet.sync(&node);

    assert_eq!(
        wallet.create_manual_transaction(vec![tx.coin_id(1, 0)], vec![]),
        Err(WalletError::UnknownCoin)
    );
} 
// ... with too much output
#[test]
fn check_manual_transaction_with_too_much_output() {
    let wallet = wallet_with_alice();
    let coin = Coin {
        value: 100,
        owner: Address::Alice,
    };
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin.clone(), coin.clone()],
    };

    assert_eq!(
        wallet.create_manual_transaction(vec![tx.coin_id(1, 0)], vec![coin]),
        Err(WalletError::UnknownCoin)
    );
}
// ... with zero output value
#[test]
fn check_manual_transaction_with_zero_output_value() {
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
    
    let coin_output = Coin {
        value: 0,
        owner: Address::Alice,
    };

    let mut wallet: Wallet = wallet_with_alice();
    wallet.sync(&node);

    assert_eq!(
        wallet.create_manual_transaction(vec![coin_id], vec![coin_output]),
        Err(WalletError::ZeroCoinValue)
    );
    
}

// Create automatic transactions
// ... with too much output
#[test]
fn check_automatic_transaction_with_too_much_output() {
    const COIN_VALUE: u64 = 100;
    let coin1 = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin1.clone()],
    };

    // Create a minimal chain to contain this transaction and sync it
    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), vec![tx]);

    let mut wallet = wallet_with_alice();
    wallet.sync(&node);

    let transaction_auto = wallet.create_automatic_transaction(Address::Bob, COIN_VALUE + 1, 0);
    assert_eq!(transaction_auto, Err(WalletError::InsufficientFunds));
}
// ... with zero change
#[test]
fn check_automatic_transaction_with_zero_change() {
    const COIN_VALUE: u64 = 100;
    let coin1 = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    //minting a coin to alice
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin1.clone()],
    };

    // Create a minimal chain to contain this transaction and sync it
    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), vec![tx]);

    let mut wallet = wallet_with_alice();
    wallet.sync(&node);

    match wallet.create_automatic_transaction(Address::Bob, 50, 50) {
        Ok(transaction) => {
            assert_eq!(transaction.inputs.len(), 1);
            assert_eq!(transaction.outputs.len(), 1);
            assert_eq!(transaction.outputs[0].value, 50);
        }
        Err(e) => {
            panic!("Error: {:?}", e);
        }
        
    }
}

// Reorg performance tests to make sure they aren't just syncing from genesis each time.
#[test]
fn reorg_performance() {
    // Create node and wallet
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice();

    // Sync a chain to height 10
    let old_b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    let old_b2_id = node.add_block_as_best(old_b1_id, vec![]);
    let old_b3_id = node.add_block_as_best(old_b2_id, vec![]);
    let old_b4_id = node.add_block_as_best(old_b3_id, vec![]);
    let old_b5_id = node.add_block_as_best(old_b4_id, vec![]);
    let old_b6_id = node.add_block_as_best(old_b5_id, vec![]);
    let old_b7_id = node.add_block_as_best(old_b6_id, vec![]);
    let old_b8_id = node.add_block_as_best(old_b7_id, vec![]);
    let old_b9_id = node.add_block_as_best(old_b8_id, vec![]);
    let _old_b10_id = node.add_block_as_best(old_b9_id, vec![]);
    node.add_block_as_best(old_b9_id, vec![]);
    wallet.sync(&node);

    // Reorg to shorter chain of length 8
    let b7_bis_id = node.add_block_as_best(old_b7_id, vec![marker_tx()]);
    let b8_bis_id = node.add_block_as_best(b7_bis_id, vec![]);
    wallet.sync(&node);

    println!("Wallet best_height: {:?}", wallet.best_height());
    println!("Wallet best_hash: {:?}", wallet.best_hash());


    assert_eq!(wallet.best_height(), 9);
    assert_eq!(wallet.best_hash(), b8_bis_id);
}

#[test]
fn deep_reorg_to_short_chain() {
    // Create node and wallet
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice();

    // Sync a chain to height 3
    let old_b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    let old_b2_id = node.add_block_as_best(old_b1_id, vec![]);
    let old_b3_id = node.add_block_as_best(old_b2_id, vec![]);
    let old_b4_id = node.add_block_as_best(old_b3_id, vec![]);
    let old_b5_id = node.add_block_as_best(old_b4_id, vec![]);
    let old_b6_id = node.add_block_as_best(old_b5_id, vec![]);
    let _old_b7_id = node.add_block_as_best(old_b6_id, vec![]);
    wallet.sync(&node);

    let b1_id = node.add_block(Block::genesis().id(), vec![marker_tx()]);
    let b2_id = node.add_block_as_best(b1_id, vec![]);
    let b3_id = node.add_block_as_best(b2_id, vec![]);
    let b4_id = node.add_block_as_best(b3_id, vec![]);
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 4);
    assert_eq!(wallet.best_hash(), b4_id);
}

#[test]
fn dont_save_coins_not_owned_by_our_wallet_addresses() {
    const COIN_VALUE: u64 = 100;
    let coin1 = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let coin2 = Coin {
        value: COIN_VALUE,
        owner: Address::Bob,
    };
    let coin3 = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin1.clone(), coin2.clone()],
    };

    let input = Input {
        coin_id: tx.coin_id(1, 1),
        signature: Signature::Invalid,
    };
    let tx2 = Transaction {
        inputs: vec![input],
        outputs: vec![coin3],
    };

    // Create node and wallet
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice();

    // Sync a chain to height 3
    let b1_id = node.add_block_as_best(Block::genesis().id(), vec![tx]);
    wallet.sync(&node);

    assert!(wallet.net_worth() == 100);

    node.add_block_as_best(b1_id, vec![tx2]);
    wallet.sync(&node);

    assert!(wallet.total_assets_of(Address::Alice) == Ok(200));
    assert!(wallet.net_worth() == 200);

}