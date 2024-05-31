//! Tests for the bonecoin wallet

use bonecoin_core::*;
use std::collections::*;
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
            },
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
            },
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
fn reorg_with_utxos_01() {
    let (mut wallet, mut node) = initial_setup();
    let coins = Vec::from_iter(wallet.all_coins_of(Address::Alice).unwrap());

    let alice_first_coin = coins.iter().find(|(_, u)| u == &200).unwrap();
    let tx = wallet.create_automatic_transaction(Address::Charlie, 190, 10);

    assert_eq!(
        tx,
        Ok(Transaction {
            inputs: vec![Input {
                coin_id: alice_first_coin.0,
                signature: Signature::Valid(Address::Alice)
            }],
            outputs: vec![Coin {
                value: 190,
                owner: Address::Charlie
            }]
        })
    );

    // consume tx
    let best = node.best_block_at_height(2).unwrap();
    node.add_block_as_best(best, vec![tx.unwrap()]);
    wallet.sync(&node);
    let last_query_count = node.how_many_queries();

    assert_eq!(wallet.total_assets_of(Address::Charlie).unwrap(), 490);
    assert_eq!(wallet.total_assets_of(Address::Alice).unwrap(), 100);

    node.add_block_as_best(best, vec![]);
    wallet.sync(&node);

    assert_eq!(node.how_many_queries() - last_query_count, 3);
    assert_eq!(wallet.total_assets_of(Address::Charlie).unwrap(), 300);
    assert_eq!(wallet.total_assets_of(Address::Alice).unwrap(), 300);
}

// Reorg performance tests to make sure they aren't just syncing from genesis each time.

// Memory performance test to make sure they aren't just keeping a snapshot of the entire UTXO set at every height.
