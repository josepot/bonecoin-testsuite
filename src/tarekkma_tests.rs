use bonecoin_core::*;
use std::collections::*;
use utxo_wallet_assignment::Wallet;

#[test]
fn total_assets_of_should_not_return_no_owned_address() {
    // https://discord.com/channels/1219966585582653471/1246066143907811368/1246112529189568555
    let wallet = Wallet::new(vec![].into_iter());

    assert_eq!(
        wallet.total_assets_of(Address::Bob),
        Err(WalletError::ForeignAddress)
    );

    assert_eq!(
        wallet.all_coins_of(Address::Bob),
        Err(WalletError::ForeignAddress)
    );

    // just get a coin id
    let dummy_tx = Transaction {
        inputs: vec![],
        outputs: vec![Coin {
            value: 100,
            owner: Address::Alice,
        }],
    };
    let dummy_coin = dummy_tx.coin_id(1, 0);

    assert_eq!(
        wallet.coin_details(&dummy_coin),
        Err(WalletError::UnknownCoin)
    );
}

#[test]
fn spend_utxo_in_same_block() {
    let mut node = MockNode::new();
    let mut wallet = Wallet::new(vec![Address::Alice, Address::Bob].into_iter());

    let coin1 = Coin {
        value: 100,
        owner: Address::Alice,
    };

    let mint_tx = Transaction {
        inputs: vec![],
        outputs: vec![coin1.clone()],
    };

    let alice_100_bucks_coin = mint_tx.coin_id(1, 0);

    let tx2 = Transaction {
        inputs: vec![Input {
            coin_id: alice_100_bucks_coin,
            signature: Signature::Valid(Address::Alice),
        }],
        outputs: vec![Coin {
            value: 100,
            owner: Address::Bob,
        }],
    };

    let bob_100_bucks_coin = tx2.coin_id(1, 0);

    let tx3 = Transaction {
        inputs: vec![Input {
            coin_id: bob_100_bucks_coin,
            // invalid signature, but wallet shouldn't care
            signature: Signature::Valid(Address::Custom(223)),
        }],
        outputs: vec![Coin {
            value: 100,
            owner: Address::Custom(100),
        }],
    };

    let block_1 = node.add_block_as_best(
        Block::genesis().id(),
        vec![mint_tx.clone(), tx2.clone(), tx3],
    );
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 1);
    assert_eq!(wallet.best_hash(), block_1);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(0));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(0));
    assert_eq!(wallet.net_worth(), 0);

    // reorg to genesis
    node.set_best(Block::genesis().id());
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 0);
    assert_eq!(wallet.best_hash(), Block::genesis().id());
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(0));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(0));
    assert_eq!(wallet.net_worth(), 0);
}

/// test sync performance with 1000 blocks
#[test]
fn perf_sync_100_blocks() {
    let mut node = MockNode::new();
    let mut wallet = Wallet::new(vec![Address::Alice, Address::Bob].into_iter());

    let mut last_block = Block::genesis().id();
    let mut block75 = last_block;
    for i in 1..=100 {
        let tx1 = Transaction {
            inputs: vec![],
            outputs: vec![Coin {
                value: 10,
                owner: Address::Alice,
            }],
        };
        let alice_coin = tx1.coin_id(i, 0);
        let tx2 = Transaction {
            inputs: vec![Input {
                coin_id: alice_coin,
                signature: Signature::Valid(Address::Alice),
            }],
            outputs: vec![
                Coin {
                    value: 2,
                    owner: Address::Bob,
                },
                Coin {
                    value: 3,
                    owner: Address::Alice,
                },
            ],
        };
        last_block = node.add_block_as_best(last_block, vec![tx1, tx2]);
        if i == 75 {
            block75 = last_block;
        }
    }

    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 100);
    assert_eq!(wallet.best_hash(), last_block);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(300));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(200));
    assert_eq!(wallet.net_worth(), 500);

    // reorg to genesis
    node.set_best(block75);
    wallet.sync(&node);

    println!("Queries: {}", node.how_many_queries());
    assert!(
        node.how_many_queries() < (75 + 100) /* we already called 100 times at least to sync to block 100 */
    );

    assert_eq!(wallet.best_height(), 75);
    assert_eq!(wallet.best_hash(), block75);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(225));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(150));
    assert_eq!(wallet.net_worth(), 375);
}

/// test sync performance with 100 blocks
#[test]
fn pref_sync_1000_blocks() {
    let mut node = MockNode::new();
    let mut wallet = Wallet::new(vec![Address::Alice, Address::Bob].into_iter());

    let mut last_block = Block::genesis().id();
    let mut block850 = last_block;
    for i in 1..=1000 {
        let tx1 = Transaction {
            inputs: vec![],
            outputs: vec![Coin {
                value: 10,
                owner: Address::Alice,
            }],
        };
        let alice_coin = tx1.coin_id(i, 0);
        let tx2 = Transaction {
            inputs: vec![Input {
                coin_id: alice_coin,
                signature: Signature::Valid(Address::Alice),
            }],
            outputs: vec![
                Coin {
                    value: 2,
                    owner: Address::Bob,
                },
                Coin {
                    value: 3,
                    owner: Address::Alice,
                },
            ],
        };
        last_block = node.add_block_as_best(last_block, vec![tx1, tx2]);
        if i == 850 {
            block850 = last_block;
        }
    }

    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 1000);
    assert_eq!(wallet.best_hash(), last_block);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(3000));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(2000));
    assert_eq!(wallet.net_worth(), 5000);

    // reorg to genesis
    node.set_best(block850);
    wallet.sync(&node);

    println!("Queries: {}", node.how_many_queries());
    assert!(
        node.how_many_queries() < (850 + 1000) /* we already called 1000 times at least to sync to block 1000 */
    );
}
