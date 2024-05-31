//! Tests for the bonecoin wallet

use bonecoin_core::*;
use std::collections::*;
use utxo_wallet_assignment::Wallet;

/// Simple helper to initialize a wallet with just one account.
fn wallet_with_alice() -> Wallet {
    Wallet::new(vec![Address::Alice].into_iter())
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

    // Make sure the UTXO is consumed
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(0));
    assert_eq!(wallet.net_worth(), 0);
    assert_eq!(wallet.all_coins_of(Address::Alice), Ok(HashSet::new()));
    // Pedagogy: It is reasonable that the wallet could provide details about
    // the coin even after it was spent. But requiring that gives away the trick of
    // tracking spent coins so you can revert them later.
    assert_eq!(wallet.coin_details(&coin_id), Err(WalletError::UnknownCoin));
}

// Track UTXOs from two transactions in a single block

// Track UTXOs to multiple users

// Create manual transaction
// ... with missing input
// ... with too much output
// ... with zero output value

// Create automatic transactions
// ... with too much output
// ... with zero change

// Reorgs with UTXOs in the chain history

// Reorg performance tests to make sure they aren't just syncing from genesis each time.

// Memory performance test to make sure they aren't just keeping a snapshot of the entire UTXO set at every height.
// Track UTXOs from two transactions in a single block
#[test]
fn extra_track_two_utxo() {
    // TODO: might be the easiest scenario
    const COIN_0_VALUE: u64 = 100;
    const COIN_1_VALUE: u64 = 200;
    const COIN_2_VALUE: u64 = 300;
    let coin = Coin {
        value: COIN_0_VALUE,
        owner: Address::Alice,
    };
    let coin_1 = Coin {
        value: COIN_1_VALUE,
        owner: Address::Alice,
    };
    let coin_2 = Coin {
        value: COIN_2_VALUE,
        owner: Address::Bob,
    };
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin.clone()],
    };
    let tx_1 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin_1.clone()],
    };
    let tx_2 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin_2.clone()],
    };
    let coin_id = tx.coin_id(1, 0);
    let coin_id_1 = tx_1.coin_id(1, 0);
    let coin_id_2 = tx_2.coin_id(1, 0);

    // Create a minimal chain to contain this transaction and sync it
    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), vec![tx, tx_1, tx_2]);

    let mut wallet = Wallet::new(vec![Address::Alice, Address::Bob].into_iter());
    wallet.sync(&node);

    // Check that the accounting is right
    assert_eq!(
        wallet.total_assets_of(Address::Alice),
        Ok(COIN_0_VALUE + COIN_1_VALUE)
    );
    assert_eq!(
        wallet.net_worth(),
        COIN_0_VALUE + COIN_1_VALUE + COIN_2_VALUE
    );
    assert_eq!(
        wallet.all_coins_of(Address::Alice),
        Ok(HashSet::from_iter([
            (coin_id, COIN_0_VALUE),
            (coin_id_1, COIN_1_VALUE)
        ]))
    );
    assert_eq!(wallet.coin_details(&coin_id), Ok(coin));
    assert_eq!(wallet.coin_details(&coin_id_1), Ok(coin_1));
    assert_eq!(wallet.coin_details(&coin_id_2), Ok(coin_2));
}

// Track UTXOs to multiple users
#[test]
fn extra_utxo_to_multiple_users() {
    // TODO: might be the easiest scenario
    const COIN_0_VALUE: u64 = 100;
    const COIN_1_VALUE: u64 = 200;
    let coin = Coin {
        value: COIN_0_VALUE,
        owner: Address::Alice,
    };
    let coin_1 = Coin {
        value: COIN_1_VALUE,
        owner: Address::Bob,
    };
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin.clone()],
    };
    let tx_1 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin_1.clone()],
    };
    let coin_id = tx.coin_id(1, 0);
    let coin_id_1 = tx_1.coin_id(1, 0);

    // Create a minimal chain to contain this transaction and sync it
    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), vec![tx, tx_1]);

    let mut wallet = Wallet::new(vec![Address::Alice, Address::Bob].into_iter());
    wallet.sync(&node);

    // Check that the accounting is right
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(COIN_0_VALUE));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(COIN_1_VALUE));
    assert_eq!(wallet.net_worth(), COIN_0_VALUE + COIN_1_VALUE);
    assert_eq!(
        wallet.all_coins_of(Address::Alice),
        Ok(HashSet::from_iter([(coin_id, COIN_0_VALUE)]))
    );
    assert_eq!(
        wallet.all_coins_of(Address::Bob),
        Ok(HashSet::from_iter([(coin_id_1, COIN_1_VALUE)]))
    );
    assert_eq!(wallet.coin_details(&coin_id), Ok(coin));
    assert_eq!(wallet.coin_details(&coin_id_1), Ok(coin_1));
}

#[test]
fn extra_best_height_and_hash() {
    let mut wallet = Wallet::new(vec![Address::Alice].into_iter());
    let mut node = MockNode::new();

    wallet.sync(&node);
    assert_eq!(wallet.best_height(), 0);
    assert_eq!(wallet.best_hash(), Block::genesis().id());

    let tx_1 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![],
    };
    let block_1 = node.add_block_as_best(Block::genesis().id(), vec![tx_1]);

    let tx_2 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![],
    };
    let block_2 = node.add_block_as_best(block_1, vec![tx_2]);
    wallet.sync(&node);
    assert_eq!(wallet.best_height(), 2);
    assert_eq!(wallet.best_hash(), block_2);

    let tx_3 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![],
    };
    let block_3 = node.add_block_as_best(block_2, vec![tx_3]);
    wallet.sync(&node);
    assert_eq!(wallet.best_height(), 3);
    assert_eq!(wallet.best_hash(), block_3);
}

#[test]
fn extra_best_height_and_hash_fork1() {
    let mut wallet = Wallet::new(vec![Address::Alice].into_iter());
    let mut node = MockNode::new();

    // Chain 1
    let tx_1 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![],
    };
    let block_1 = node.add_block_as_best(Block::genesis().id(), vec![tx_1]);

    let tx_2 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![],
    };
    let block_2 = node.add_block_as_best(block_1, vec![tx_2]);

    let tx_3 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![],
    };
    let block_3 = node.add_block_as_best(block_2, vec![tx_3]);
    wallet.sync(&node);
    assert_eq!(wallet.best_height(), 3);
    assert_eq!(wallet.best_hash(), block_3);

    // Chain 2, this chain will be longer
    let tx_2 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 0,
            owner: Address::Alice,
        }],
    };
    let block_2 = node.add_block_as_best(block_1, vec![tx_2]);

    let tx_3 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 0,
            owner: Address::Alice,
        }],
    };
    let block_3 = node.add_block_as_best(block_2, vec![tx_3]);

    let tx_4 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 0,
            owner: Address::Alice,
        }],
    };
    let block_4 = node.add_block_as_best(block_3, vec![tx_4]);
    wallet.sync(&node);
    assert_eq!(wallet.best_height(), 4);
    assert_eq!(wallet.best_hash(), block_4);
}

#[test]
fn extra_best_height_and_hash_fork2() {
    let mut wallet = Wallet::new(vec![Address::Alice].into_iter());
    let mut node = MockNode::new();

    // Chain 1
    let tx_1 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![],
    };
    let block_1 = node.add_block_as_best(Block::genesis().id(), vec![tx_1]);

    let tx_2 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![],
    };
    let block_2 = node.add_block_as_best(block_1, vec![tx_2]);

    let tx_3 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![],
    };
    let block_3 = node.add_block_as_best(block_2, vec![tx_3]);
    wallet.sync(&node);
    assert_eq!(wallet.best_height(), 3);
    assert_eq!(wallet.best_hash(), block_3);

    // Chain 2, this chain will be shorter
    let tx_2 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 0,
            owner: Address::Alice,
        }],
    };
    let block_2 = node.add_block_as_best(block_1, vec![tx_2]);
    wallet.sync(&node);
    assert_eq!(wallet.best_height(), 2);
    assert_eq!(wallet.best_hash(), block_2);
}

#[test]
fn extra_total_assets_of_simple() {
    let mut wallet = Wallet::new(vec![Address::Alice, Address::Bob].into_iter());
    let mut node = MockNode::new();

    let tx_1a = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![
            Coin {
                value: 12,
                owner: Address::Alice,
            },
            Coin {
                value: 15,
                owner: Address::Bob,
            },
            Coin {
                value: 53,
                owner: Address::Alice,
            },
        ],
    };
    let tx_1b = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 10,
            owner: Address::Alice,
        }],
    };
    let block_1 = node.add_block_as_best(Block::genesis().id(), vec![tx_1a, tx_1b]);

    let tx_2 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![
            Coin {
                value: 3,
                owner: Address::Alice,
            },
            Coin {
                value: 7,
                owner: Address::Bob,
            },
            Coin {
                value: 27,
                owner: Address::Bob,
            },
        ],
    };
    let block_2 = node.add_block_as_best(block_1, vec![tx_2]);

    let tx_3a = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 89,
            owner: Address::Alice,
        }],
    };
    let tx_3b = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![
            Coin {
                value: 63,
                owner: Address::Bob,
            },
            Coin {
                value: 20,
                owner: Address::Alice,
            },
        ],
    };
    let tx_3c = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 10,
            owner: Address::Alice,
        }],
    };
    node.add_block_as_best(block_2, vec![tx_3a, tx_3b, tx_3c]);
    wallet.sync(&node);
    assert_eq!(
        wallet.total_assets_of(Address::Alice),
        Ok(12 + 53 + 10 + 3 + 89 + 20 + 10)
    );
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(15 + 7 + 27 + 63));
}

#[test]
fn extra_total_assets_of_fork() {
    let mut wallet = Wallet::new(vec![Address::Alice, Address::Bob].into_iter());
    let mut node = MockNode::new();

    // First chain
    let tx_1a = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![
            Coin {
                value: 12,
                owner: Address::Alice,
            },
            Coin {
                value: 15,
                owner: Address::Bob,
            },
            Coin {
                value: 53,
                owner: Address::Alice,
            },
        ],
    };
    let tx_1b = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 10,
            owner: Address::Alice,
        }],
    };
    let block_1 = node.add_block_as_best(Block::genesis().id(), vec![tx_1a, tx_1b]);

    let tx_2 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![
            Coin {
                value: 3,
                owner: Address::Alice,
            },
            Coin {
                value: 7,
                owner: Address::Bob,
            },
            Coin {
                value: 27,
                owner: Address::Bob,
            },
        ],
    };
    let block_2 = node.add_block_as_best(block_1, vec![tx_2]);

    let tx_3a = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 89,
            owner: Address::Alice,
        }],
    };
    let tx_3b = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![
            Coin {
                value: 63,
                owner: Address::Bob,
            },
            Coin {
                value: 20,
                owner: Address::Alice,
            },
        ],
    };
    let tx_3c = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 10,
            owner: Address::Alice,
        }],
    };
    node.add_block_as_best(block_2, vec![tx_3a, tx_3b, tx_3c]);
    wallet.sync(&node);
    assert_eq!(
        wallet.total_assets_of(Address::Alice),
        Ok(12 + 53 + 10 + 3 + 89 + 20 + 10)
    );
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(15 + 7 + 27 + 63));

    // Second chain
    let tx_3 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 111,
            owner: Address::Bob,
        }],
    };
    let block_3 = node.add_block_as_best(block_2, vec![tx_3]);

    let tx_4 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 92,
            owner: Address::Alice,
        }],
    };
    node.add_block_as_best(block_3, vec![tx_4]);
    wallet.sync(&node);

    assert_eq!(
        wallet.total_assets_of(Address::Alice),
        Ok(12 + 53 + 10 + 3 + 92)
    );
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(15 + 7 + 27 + 111));
}

#[test]
fn extra_total_assets_of_overflow() {
    let mut wallet = Wallet::new(vec![Address::Alice, Address::Bob].into_iter());
    let mut node = MockNode::new();

    // First chain
    let tx_1 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![
            Coin {
                value: 12,
                owner: Address::Alice,
            },
            Coin {
                value: u64::MAX,
                owner: Address::Alice,
            },
            Coin {
                value: 53,
                owner: Address::Alice,
            },
        ],
    };
    node.add_block_as_best(Block::genesis().id(), vec![tx_1]);
    wallet.sync(&node);
    assert_eq!(wallet.net_worth(), u64::MAX);
}

#[test]
fn extra_total_assets_of_empty() {
    let wallet = Wallet::new(vec![Address::Alice].into_iter());
    let total_assets = wallet.total_assets_of(Address::Alice);
    assert_eq!(total_assets, Ok(0));
}

#[test]
fn extra_total_assets_of_foreign_address() {
    let wallet = Wallet::new(vec![].into_iter());
    let total_assets = wallet.total_assets_of(Address::Alice);
    assert_eq!(total_assets, Err(WalletError::ForeignAddress));
}

#[test]
fn extra_net_worth_simple() {
    let mut wallet = Wallet::new(vec![Address::Alice, Address::Bob].into_iter());
    let mut node = MockNode::new();

    let tx_1a = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![
            Coin {
                value: 12,
                owner: Address::Alice,
            },
            Coin {
                value: 15,
                owner: Address::Bob,
            },
            Coin {
                value: 53,
                owner: Address::Alice,
            },
        ],
    };
    let tx_1b = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 10,
            owner: Address::Alice,
        }],
    };
    let block_1 = node.add_block_as_best(Block::genesis().id(), vec![tx_1a, tx_1b]);

    let tx_2 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![
            Coin {
                value: 3,
                owner: Address::Alice,
            },
            Coin {
                value: 7,
                owner: Address::Bob,
            },
            Coin {
                value: 27,
                owner: Address::Bob,
            },
        ],
    };
    let block_2 = node.add_block_as_best(block_1, vec![tx_2]);

    let tx_3a = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 89,
            owner: Address::Alice,
        }],
    };
    let tx_3b = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![
            Coin {
                value: 63,
                owner: Address::Bob,
            },
            Coin {
                value: 20,
                owner: Address::Alice,
            },
        ],
    };
    let tx_3c = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 10,
            owner: Address::Alice,
        }],
    };
    node.add_block_as_best(block_2, vec![tx_3a, tx_3b, tx_3c]);
    wallet.sync(&node);
    assert_eq!(
        wallet.net_worth(),
        12 + 53 + 10 + 3 + 89 + 20 + 10 + 15 + 7 + 27 + 63
    );
}

#[test]
fn extra_net_worth_fork() {
    let mut wallet = Wallet::new(vec![Address::Alice, Address::Bob].into_iter());
    let mut node = MockNode::new();

    // First chain
    let tx_1a = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![
            Coin {
                value: 12,
                owner: Address::Alice,
            },
            Coin {
                value: 15,
                owner: Address::Bob,
            },
            Coin {
                value: 53,
                owner: Address::Alice,
            },
        ],
    };
    let tx_1b = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 10,
            owner: Address::Alice,
        }],
    };
    let block_1 = node.add_block_as_best(Block::genesis().id(), vec![tx_1a, tx_1b]);

    let tx_2 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![
            Coin {
                value: 3,
                owner: Address::Alice,
            },
            Coin {
                value: 7,
                owner: Address::Bob,
            },
            Coin {
                value: 27,
                owner: Address::Bob,
            },
        ],
    };
    let block_2 = node.add_block_as_best(block_1, vec![tx_2]);

    let tx_3a = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 89,
            owner: Address::Alice,
        }],
    };
    let tx_3b = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![
            Coin {
                value: 63,
                owner: Address::Bob,
            },
            Coin {
                value: 20,
                owner: Address::Alice,
            },
        ],
    };
    let tx_3c = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 10,
            owner: Address::Alice,
        }],
    };
    node.add_block_as_best(block_2, vec![tx_3a, tx_3b, tx_3c]);
    wallet.sync(&node);
    assert_eq!(
        wallet.net_worth(),
        12 + 53 + 10 + 3 + 89 + 20 + 10 + 15 + 7 + 27 + 63
    );

    // Second chain
    let tx_3 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 111,
            owner: Address::Bob,
        }],
    };
    let block_3 = node.add_block_as_best(block_2, vec![tx_3]);

    let tx_4 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 92,
            owner: Address::Alice,
        }],
    };
    node.add_block_as_best(block_3, vec![tx_4]);
    wallet.sync(&node);

    assert_eq!(
        wallet.net_worth(),
        12 + 53 + 10 + 3 + 92 + 15 + 7 + 27 + 111
    );
}

#[test]
fn extra_net_worth_overflow() {
    let mut wallet = Wallet::new(vec![Address::Alice, Address::Bob].into_iter());
    let mut node = MockNode::new();

    // First chain
    let tx_1 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![
            Coin {
                value: 12,
                owner: Address::Alice,
            },
            Coin {
                value: u64::MAX,
                owner: Address::Alice,
            },
            Coin {
                value: 53,
                owner: Address::Alice,
            },
        ],
    };
    node.add_block_as_best(Block::genesis().id(), vec![tx_1]);
    wallet.sync(&node);
    assert_eq!(wallet.net_worth(), u64::MAX);
}

#[test]
fn extra_total_net_worth_empty() {
    let wallet = Wallet::new(vec![].into_iter());
    assert_eq!(wallet.net_worth(), 0);
}

// Create manual transaction
#[test]
fn extra_create_manual_transaction() {
    const COIN_0_VALUE: u64 = 200;
    const COIN_1_VALUE: u64 = 100;

    let coin_0 = Coin {
        value: COIN_0_VALUE,
        owner: Address::Alice,
    };

    let coin_1 = Coin {
        value: COIN_1_VALUE,
        owner: Address::Bob,
    };

    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin_0.clone()],
    };

    let coin_id = tx.coin_id(1, 0);

    let mut wallet = Wallet::new(vec![Address::Alice, Address::Bob].into_iter());
    // Create a minimal chain to contain this transaction and sync it
    let mut node = MockNode::new();
    let node_1 = node.add_block_as_best(Block::genesis().id(), vec![tx]);
    wallet.sync(&node);

    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(COIN_0_VALUE));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(0));

    let manual_tx = wallet
        .create_manual_transaction(vec![coin_id], vec![coin_1.clone()])
        .unwrap();
    println!("manual_tx: {:?}", manual_tx);
    node.add_block_as_best(node_1, vec![manual_tx]);
    wallet.sync(&node);

    // Check that the accounting is right
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(0));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(COIN_1_VALUE));
    assert_eq!(wallet.net_worth(), COIN_1_VALUE);
}

#[test]
fn extra_automatic_transaction_simple() {
    // Alice is the sending address in this case and Bob receives the automatic tx.
    let mut wallet = Wallet::new(vec![Address::Alice, Address::Bob].into_iter());
    let mut node = MockNode::new();

    // Alice starts with 85 bones.
    let tx_0 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![
            Coin {
                value: 20,
                owner: Address::Alice,
            },
            Coin {
                value: 12,
                owner: Address::Alice,
            },
            Coin {
                value: 53,
                owner: Address::Alice,
            },
        ],
    };

    // Synchronises bone with the wallet.
    let best_block = node.add_block_as_best(Block::genesis().id(), vec![tx_0]);
    wallet.sync(&node);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(85));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(0));

    // Creates the automatic transaction. This should not fail.
    let tx_auto = wallet.create_automatic_transaction(Address::Bob, 80, 3);
    assert!(tx_auto.is_ok());

    node.add_block_as_best(best_block, vec![tx_auto.unwrap()]);
    wallet.sync(&node);

    // Because the specs only specify to "sends the remaining amount back to an address" in the
    // wallet, this means either Alice or Bob could receive the tip. This is especially
    // pertinent if you are using a HashMap or HashSet to store your address, as these have
    // non-deterministic ordering over multiple tests.
    let assets_alice = wallet.total_assets_of(Address::Alice);
    let assets_bob = wallet.total_assets_of(Address::Bob);
    assert!(
        (assets_bob == Ok(80) && assets_alice == Ok(2))
            || (assets_bob == Ok(82) && assets_alice == Ok(0))
    );
}

#[test]
fn extra_automatic_transaction_invalid() {
    // Alice is the sending address.
    let mut wallet = Wallet::new(vec![Address::Alice].into_iter());
    let mut node = MockNode::new();

    // Alice starts with 85 bones.
    let tx_0 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![
            Coin {
                value: 20,
                owner: Address::Alice,
            },
            Coin {
                value: 12,
                owner: Address::Alice,
            },
            Coin {
                value: 53,
                owner: Address::Alice,
            },
        ],
    };

    // Synchronises bone with the wallet.
    node.add_block_as_best(Block::genesis().id(), vec![tx_0]);
    wallet.sync(&node);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(85));

    // Creates the automatic transaction requesting more money than Alice has. This should fail.
    let tx_auto = wallet.create_automatic_transaction(Address::Bob, 100, 3);
    assert_eq!(tx_auto, Err(WalletError::InsufficientFunds));
}

#[test]
fn extra_automatic_transaction_no_owned_address() {
    // Alice is the sending address.
    let wallet = Wallet::new(vec![].into_iter());

    // This should fail because our wallet was not given any address to own on initialisation.
    let tx_auto = wallet.create_automatic_transaction(Address::Bob, 100, 3);
    assert_eq!(tx_auto, Err(WalletError::NoOwnedAddresses));
}

#[test]
fn extra_automatic_transaction_zero_coin_value() {
    // Alice is the sending address.
    let mut wallet = Wallet::new(vec![Address::Alice].into_iter());
    let mut node = MockNode::new();

    // Alice starts with 85 bones.
    let tx_0 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![
            Coin {
                value: 20,
                owner: Address::Alice,
            },
            Coin {
                value: 12,
                owner: Address::Alice,
            },
            Coin {
                value: 53,
                owner: Address::Alice,
            },
        ],
    };

    // Synchronises bone with the wallet.
    node.add_block_as_best(Block::genesis().id(), vec![tx_0]);
    wallet.sync(&node);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(85));

    // We are asking for an auto transaction of value 0. This is not alowed as this would be a
    // possible attack vector to flood the network with transactions.
    let tx_auto = wallet.create_automatic_transaction(Address::Bob, 0, 3);
    assert_eq!(tx_auto, Err(WalletError::ZeroCoinValue));
}

#[test]
// WARN: this test is by all regards overkill and should most likely not be part of the actual
// testing battery. Still if you feel paranoid (like me) and do not want to assume anything, this
// can be a fun thing to puzzle out :)
fn extra_automatic_transaction_overflow() {
    // Alice is the sending address.
    let mut wallet = Wallet::new(vec![Address::Alice].into_iter());
    let mut node = MockNode::new();

    let tx_0 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![
            Coin {
                value: 16,
                owner: Address::Alice,
            },
            Coin {
                value: u64::MAX,
                owner: Address::Alice,
            },
        ],
    };

    // Synchronises bone with the wallet.
    let best_block = node.add_block_as_best(Block::genesis().id(), vec![tx_0]);
    wallet.sync(&node);

    // Create and apply automatic transaction.
    // WARN: this should not fail as Alice has sufficient funds in her account.
    let tx_auto = wallet.create_automatic_transaction(Address::Bob, 17, 3);
    assert!(tx_auto.is_ok());
    node.add_block_as_best(best_block, vec![tx_auto.unwrap()]);
    wallet.sync(&node);

    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(u64::MAX - 4));
}
