//! Written by sinzii

use std::collections::*;
use bonecoin_core::*;
use utxo_wallet_assignment::Wallet;

/// Simple helper to initialize a wallet with just one account.
fn wallet_with_alice() -> Wallet {
    Wallet::new(vec![Address::Alice].into_iter())
}

fn wallet_with_multiple_users() -> Wallet {
    Wallet::new(vec![Address::Alice, Address::Bob, Address::Charlie].into_iter())
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

fn initial_setup() -> (impl WalletApi, MockNode) {
    // All coins will be valued the same in this test
    // We start by minting a coin to alice
    let tx_mint_1 = Transaction {
        inputs: vec![],
        outputs: vec![
            Coin {
                value: 100,
                owner: Address::Alice,
            },
            Coin {
                value: 100,
                owner: Address::Bob,
            },
            Coin {
                value: 100,
                owner: Address::Charlie,
            }
        ],
    };

    let tx_mint_2 = Transaction {
        inputs: vec![],
        outputs: vec![
            Coin {
                value: 200,
                owner: Address::Alice,
            },
            Coin {
                value: 200,
                owner: Address::Bob,
            },
            Coin {
                value: 200,
                owner: Address::Charlie,
            }
        ],
    };

    let mut node = MockNode::new();
    let b1_id = node.add_block_as_best(Block::genesis().id(), vec![tx_mint_1.clone()]);
    let _b2_id = node.add_block_as_best(b1_id, vec![tx_mint_2.clone()]);

    let mut wallet = wallet_with_multiple_users();
    wallet.sync(&node);

    (wallet, node)
}

#[test]
fn reorg_in_the_middle_with_tx_changes() {
    let (mut wallet, mut node) = initial_setup();

    wallet.sync(&node);
    assert_eq!(wallet.total_assets_of(Address::Charlie), Ok(300));
    assert_eq!(wallet.best_height(), 2);

    let tx = Transaction {
        inputs: vec![],
        outputs: vec![Coin { value: 200, owner: Address::Charlie }]
    };

    let b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    let b2_id = node.add_block_as_best(b1_id, vec![]);
    let b3_id = node.add_block_as_best(b2_id, vec![tx]);
    wallet.sync(&node);
    assert_eq!(wallet.best_hash(), b3_id);
    assert_eq!(wallet.best_height(), 3);
    assert_eq!(wallet.total_assets_of(Address::Charlie), Ok(200));

    let new_b3_id = node.add_block_as_best(b2_id, vec![]);
    let new_b4_id = node.add_block_as_best(new_b3_id, vec![marker_tx()]);
    let new_b5_id = node.add_block_as_best(new_b4_id, vec![marker_tx()]);
    wallet.sync(&node);

    assert_eq!(wallet.best_hash(), new_b5_id);
    assert_eq!(wallet.best_height(), 5);
    assert_eq!(wallet.total_assets_of(Address::Charlie), Ok(0));
}


// Reorg performance tests to make sure they aren't just syncing from genesis each time.

// Memory performance test to make sure they aren't just keeping a snapshot of the entire UTXO set at every height.
