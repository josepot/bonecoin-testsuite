use bonecoin_core::*;
use std::collections::*;
use utxo_wallet_assignment::Wallet;

fn wallet_with_alice_and_bob() -> Wallet {
    Wallet::new(vec![Address::Alice, Address::Bob].into_iter())
}

#[test]
fn reorg_hard_test_hehe() {
    // Create node and wallet
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice_and_bob();
    // Mint some coins
    let coin1 = Coin {
        value: 100,
        owner: Address::Alice,
    };
    let coin2 = Coin {
        value: 90,
        owner: Address::Alice,
    };
    let coin3 = Coin {
        value: 80,
        owner: Address::Bob,
    };
    let coin4 = Coin {
        value: 70,
        owner: Address::Bob,
    };
    let coin5 = Coin {
        value: 800,
        owner: Address::Alice,
    };

    let coin6 = Coin {
        value: 15,
        owner: Address::Alice,
    };
    let mint_tx = Transaction {
        inputs: vec![],
        outputs: vec![
            coin1.clone(),
            coin2.clone(),
            coin3.clone(),
            coin4.clone(),
            coin5.clone(),
            coin6.clone(),
        ],
    };
    let alice_100_bucks_coin = mint_tx.coin_id(1, 0);
    let alice_90_bucks_coin = mint_tx.coin_id(1, 1);
    let bob_80_bucks_coin = mint_tx.coin_id(1, 2);
    let bob_70_bucks_coin = mint_tx.coin_id(1, 3);
    let alice_800_bucks_coin = mint_tx.coin_id(1, 4);
    let alice_15_bucks_coin = mint_tx.coin_id(1, 5);
    let block_1 = node.add_block(Block::genesis().id(), vec![mint_tx]);
    let tx1 = Transaction {
        inputs: vec![Input {
            coin_id: alice_100_bucks_coin,
            signature: Signature::Invalid,
        }],
        outputs: vec![Coin {
            value: 50,
            owner: Address::Bob,
        }],
    };

    let bob_coin_created_at_block_2 = tx1.coin_id(2, 0);
    let block2 = node.add_block(block_1, vec![tx1]);
    let tx2_1 = Transaction {
        inputs: vec![
            Input {
                coin_id: bob_80_bucks_coin,
                signature: Signature::Invalid,
            },
            Input {
                coin_id: alice_800_bucks_coin,
                signature: Signature::Invalid,
            },
        ],
        outputs: vec![Coin {
            value: 880,
            owner: Address::Alice,
        }],
    };
    let alice_coin_created_and_destroyed_at_block_3 = tx2_1.coin_id(3, 0);
    let tx2_2 = Transaction {
        inputs: vec![Input {
            coin_id: alice_coin_created_and_destroyed_at_block_3,
            signature: Signature::Invalid,
        }],
        outputs: vec![Coin {
            value: 300,
            owner: Address::Bob,
        }],
    };
    let bob_coin_created_at_block_3 = tx2_2.coin_id(3, 0);
    let block3 = node.add_block(block2, vec![tx2_1, tx2_2]);
    let tx3 = Transaction {
        inputs: vec![Input {
            coin_id: alice_15_bucks_coin,
            signature: Signature::Invalid,
        }],
        outputs: vec![Coin {
            value: 10,
            owner: Address::Alice,
        }],
    };
    let alice_coin_created_at_block_4 = tx3.coin_id(4, 0);
    let block_4 = node.add_block_as_best(block3, vec![tx3]);
    // Sync the wallet to a blockchain with 5 blocks
    wallet.sync(&node);
    // Check we've synched correctly
    assert_eq!(4, wallet.best_height());
    assert_eq!(block_4, wallet.best_hash());
    assert_eq!(
        Ok(HashSet::from([
            (alice_90_bucks_coin, 90),
            (alice_coin_created_at_block_4, 10)
        ])),
        wallet.all_coins_of(Address::Alice)
    );
    assert_eq!(
        Ok(HashSet::from([
            (bob_70_bucks_coin, 70),
            (bob_coin_created_at_block_2, 50),
            (bob_coin_created_at_block_3, 300)
        ])),
        wallet.all_coins_of(Address::Bob)
    );
    assert_eq!(Ok(100), wallet.total_assets_of(Address::Alice));
    assert_eq!(Ok(420), wallet.total_assets_of(Address::Bob));
    assert_eq!(520, wallet.net_worth());


    // Let's get rid of the last two blocks, to check that the created and destroyed coin at block 3 isn't in our wallet. It was created in the same block!

    node.add_block_as_best(block2, vec![marker_tx()]);
    wallet.sync(&node);
    assert!(wallet.coin_details(&alice_coin_created_and_destroyed_at_block_3).is_err());

    // Let's reorg the last_two_blocks
    let tx2_1 = Transaction {
        inputs: vec![
            Input {
                coin_id: bob_80_bucks_coin,
                signature: Signature::Invalid,
            },
            Input {
                coin_id: alice_800_bucks_coin,
                signature: Signature::Invalid,
            },
        ],
        outputs: vec![Coin {
            value: 880,
            owner: Address::Alice,
        }],
    };

    let alice_coin_created_at_block_3 = tx2_1.coin_id(3, 0);
    let block_3 = node.add_block(block2, vec![tx2_1]);
    let tx3 = Transaction {
        inputs: vec![Input {
            coin_id: alice_90_bucks_coin,
            signature: Signature::Invalid,
        }],
        outputs: vec![Coin {
            value: 30,
            owner: Address::Alice,
        }],
    };
    let alice_coin_created_at_block_4 = tx3.coin_id(4, 0);
    let block_4 = node.add_block_as_best(block_3, vec![tx3]);

    // Sync the reorg
    wallet.sync(&node);
    assert_eq!(4, wallet.best_height());
    assert_eq!(block_4, wallet.best_hash());
    // this two are actually equal. Prior the reorg, we have spent it. Now, BOOM, 880 bucks up man
    assert_eq!(
        alice_coin_created_and_destroyed_at_block_3,
        alice_coin_created_at_block_3
    );

    assert_eq!(
        Ok(HashSet::from([
            (alice_15_bucks_coin, 15),
            (alice_coin_created_at_block_4, 30),
            (alice_coin_created_at_block_3, 880)
        ])),
        wallet.all_coins_of(Address::Alice)
    );
    assert_eq!(
        Ok(HashSet::from([
            (bob_70_bucks_coin, 70),
            (bob_coin_created_at_block_2, 50)
        ])),
        wallet.all_coins_of(Address::Bob)
    );

    assert_eq!(Ok(925), wallet.total_assets_of(Address::Alice));
    assert_eq!(Ok(120), wallet.total_assets_of(Address::Bob));
    assert_eq!(1045, wallet.net_worth());
}
