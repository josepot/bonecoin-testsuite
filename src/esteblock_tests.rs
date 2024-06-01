// Written by esteblock

use bonecoin_core::*;
use utxo_wallet_assignment::Wallet;
use std::collections::HashSet;

/// Simple helper to initialize a wallet with just one account.
fn wallet_with_alice() -> Wallet {
    Wallet::new(vec![Address::Alice].into_iter())
}

/// Simple helper to initialize a wallet with just one account.
fn wallet_with_alice_and_bob() -> Wallet {
    Wallet::new(vec![Address::Alice, Address::Bob].into_iter())
}


fn marker_tx(value: u64) -> Transaction {
    Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value,
            owner: Address::Custom(value),
        }],
    }
}

//                         Old_B4 (discard)   -     Old_B5 (discard) - Old_B6 (discard)
//                       /
//     G - B1 -- B2 -- B3
//                       \  B4    --  B5 (should reorg the chain here)
#[test]
fn reports_correct_ancestors_even_after_reorg_in_the_middle() {
    let mut node = MockNode::new();

    // Build the permanent blocks
    let b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    let b2_id = node.add_block_as_best(b1_id, vec![]);
    let b3_id = node.add_block_as_best(b2_id, vec![]);

    // Now the old blcoks that will be discarted
    let old_b4_id = node.add_block_as_best(b3_id, vec![]);
    let old_b5_id = node.add_block_as_best(old_b4_id, vec![marker_tx(0123)]);
    let old_b6_id = node.add_block_as_best(old_b5_id, vec![marker_tx(456)]);

    // assert_eq!(node.best_block, old_b6_id);
    assert_eq!(node.best_block_at_height(0), Some(Block::genesis().id()));
    assert_eq!(node.best_block_at_height(1), Some(b1_id));
    assert_eq!(node.best_block_at_height(2), Some(b2_id));
    assert_eq!(node.best_block_at_height(3), Some(b3_id));
    assert_eq!(node.best_block_at_height(4), Some(old_b4_id));
    assert_eq!(node.best_block_at_height(5), Some(old_b5_id));
    assert_eq!(node.best_block_at_height(6), Some(old_b6_id));
    assert_eq!(node.best_block_at_height(7), None);
    // Now build a "new" chain that will eventually become best, forked from B3
    // In this case, I make it best at height 5: shorter than the previous best.
    // This emphasizes that there is no longest chain rule.

    let b4_id = node.add_block_as_best(b3_id, vec![marker_tx(789)]);
    let b5_id = node.add_block_as_best(b4_id, vec![marker_tx(1011)]);

    // MODIFIED: commented this out
    // assert_eq!(node.best_block, b5_id);
    assert_eq!(node.best_block_at_height(0), Some(Block::genesis().id()));
    assert_eq!(node.best_block_at_height(1), Some(b1_id));
    assert_eq!(node.best_block_at_height(2), Some(b2_id));
    assert_eq!(node.best_block_at_height(3), Some(b3_id));
    assert_eq!(node.best_block_at_height(4), Some(b4_id));
    assert_eq!(node.best_block_at_height(5), Some(b5_id));
    assert_eq!(node.best_block_at_height(6), None);
}


#[test]
fn reports_correct_ancestors_even_after_reorg_in_the_middle_with_atomic() {
    let mut node = MockNode::new();
    let b1_id = node.add_block_as_best(Block::genesis().id(), vec![]); // B1 is EMPTY

    let coin_0 = Coin {
        value: 100,
        owner: Address::Alice,
    };
    let tx_mint = Transaction {
        inputs: vec![],
        outputs: vec![coin_0.clone()],
    };
    let coin_id_0 = tx_mint.coin_id(2, 0);

    let b2_id = node.add_block_as_best(b1_id, vec![tx_mint]); //B2 WITH MINT TX

    
    let coin_1 = Coin {
        value: 4,
        owner: Address::Bob,
    };
    let coin_2 = Coin {
        value: 6,
        owner: Address::Bob,
    };
    let coin_3 = Coin {
        value: 90,
        owner: Address::Alice,
    };
    let tx_alice_bob_0 = Transaction {
        inputs: vec![Input{coin_id: coin_id_0, signature: Signature::Invalid}],
        outputs: vec![coin_1.clone(), coin_2.clone(), coin_3.clone().clone()],
    };

    let coin_id_1 = tx_alice_bob_0.coin_id(3, 0);
    let coin_id_2 = tx_alice_bob_0.coin_id(3, 1);
    let coin_id_3 = tx_alice_bob_0.coin_id(3, 2);

    let b3_id = node.add_block_as_best(b2_id, vec![tx_alice_bob_0.clone()]); // B3 WITH TXS

    let mut wallet = wallet_with_alice_and_bob();
    wallet.sync(&node);
    assert_eq!(wallet.best_height(), 3);
    assert_eq!(wallet.best_hash(), b3_id);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(90));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(10));
    assert_eq!(wallet.net_worth(), 100);
    
    let mut expected_alice_hash_set = HashSet::new();
    expected_alice_hash_set.insert((coin_id_3, 90 as u64));
    assert_eq!(wallet.all_coins_of(Address::Alice).unwrap(), expected_alice_hash_set);

    let mut expected_bob_hash_set = HashSet::new();
    expected_bob_hash_set.insert((coin_id_1, 4 as u64));
    expected_bob_hash_set.insert((coin_id_2, 6 as u64));
    assert_eq!(wallet.all_coins_of(Address::Bob), Ok(expected_bob_hash_set));

    assert_eq!(wallet.coin_details(&coin_id_0), Err(WalletError::UnknownCoin));
    assert_eq!(wallet.coin_details(&coin_id_1), Ok(coin_1));
    assert_eq!(wallet.coin_details(&coin_id_2), Ok(coin_2.clone()));
    assert_eq!(wallet.coin_details(&coin_id_3), Ok(coin_3.clone()));

    // Now the old blcoks that will be discarted
    let old_b4_id = node.add_block_as_best(b3_id, vec![]); //bob 9, alice 91
    let old_b5_id = node.add_block_as_best(old_b4_id, vec![marker_tx(0123)]);

    let coin_4 = Coin {
        value: 1,
        owner: Address::Alice,
    };
    let coin_5 = Coin {
        value: 3,
        owner: Address::Bob,
    };
    let tx_alice_bob_1 = Transaction {
        inputs: vec![Input{coin_id: coin_id_1, signature: Signature::Invalid}],
        outputs: vec![coin_4.clone(), coin_5.clone()],
    };


    let coin_id_4 = tx_alice_bob_1.coin_id(6, 0);
    let coin_id_5 = tx_alice_bob_1.coin_id(6, 1);
    
    let coin_6 = Coin {
        value: 73,
        owner: Address::Alice,
    };
    let coin_7 = Coin {
        value: 20,
        owner: Address::Bob,
    };
    let tx_alice_bob_2 = Transaction {
        inputs: vec![
            Input{coin_id: coin_id_3, signature: Signature::Invalid}, 
            Input{coin_id: coin_id_5, signature: Signature::Invalid} ],
        outputs: vec![coin_6.clone(), coin_7.clone()],
    };
    let coin_id_6 = tx_alice_bob_2.coin_id(6, 0);
    let coin_id_7 = tx_alice_bob_2.coin_id(6, 1);
    let old_b6_id = node.add_block_as_best(old_b5_id, vec![
        tx_alice_bob_1, tx_alice_bob_2]);//bob 29, alice 713
    // assert_eq!(node.best_block, old_b6_id);
    assert_eq!(node.best_block_at_height(0), Some(Block::genesis().id()));
    assert_eq!(node.best_block_at_height(1), Some(b1_id));
    assert_eq!(node.best_block_at_height(2), Some(b2_id));
    assert_eq!(node.best_block_at_height(3), Some(b3_id));
    assert_eq!(node.best_block_at_height(4), Some(old_b4_id));
    assert_eq!(node.best_block_at_height(5), Some(old_b5_id));
    assert_eq!(node.best_block_at_height(6), Some(old_b6_id));
    assert_eq!(node.best_block_at_height(7), None);

    wallet.sync(&node);
    assert_eq!(wallet.best_height(), 6);
    assert_eq!(wallet.best_hash(), old_b6_id);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(74));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(26));
    assert_eq!(wallet.net_worth(), 100);
    
    let mut expected_alice_hash_set = HashSet::new();
    expected_alice_hash_set.insert((coin_id_6, 73 as u64));
    expected_alice_hash_set.insert((coin_id_4, 1 as u64));
    assert_eq!(wallet.all_coins_of(Address::Alice).unwrap(), expected_alice_hash_set);

    let mut expected_bob_hash_set = HashSet::new();
    expected_bob_hash_set.insert((coin_id_7, 20 as u64));
    expected_bob_hash_set.insert((coin_id_2, 6 as u64));
    // expected_bob_hash_set.insert((coin_id_5, 3 as u64));
    assert_eq!(wallet.all_coins_of(Address::Bob), Ok(expected_bob_hash_set));


    assert_eq!(wallet.coin_details(&coin_id_0), Err(WalletError::UnknownCoin));
    assert_eq!(wallet.coin_details(&coin_id_1), Err(WalletError::UnknownCoin));
    assert_eq!(wallet.coin_details(&coin_id_2), Ok(coin_2));
    assert_eq!(wallet.coin_details(&coin_id_3), Err(WalletError::UnknownCoin));
    assert_eq!(wallet.coin_details(&coin_id_4), Ok(coin_4));
    assert_eq!(wallet.coin_details(&coin_id_5), Err(WalletError::UnknownCoin));
    assert_eq!(wallet.coin_details(&coin_id_6), Ok(coin_6));
    assert_eq!(wallet.coin_details(&coin_id_7), Ok(coin_7));

  
     let coin_8 = Coin {
        value: 7,
        owner: Address::Alice,
    };
    let coin_9 = Coin {
        value: 3,
        owner: Address::Bob,
    };

    let tx_alice_bob_3 = Transaction {
        inputs: vec![Input{coin_id: coin_id_1, signature: Signature::Invalid}, Input{coin_id: coin_id_2, signature: Signature::Invalid}],
        outputs: vec![coin_8.clone(), coin_9.clone()],
    };
    let coin_id_8 = tx_alice_bob_3.coin_id(4, 0);
    let coin_id_9 = tx_alice_bob_3.coin_id(4, 1);
    let b4_id = node.add_block_as_best(b3_id, vec![tx_alice_bob_3]);//bob 29, alice 71
    

    let b5_id = node.add_block_as_best(b4_id, vec![marker_tx(1011)]);

    // assert_eq!(node.best_block, b5_id);
    assert_eq!(node.best_block_at_height(0), Some(Block::genesis().id()));
    assert_eq!(node.best_block_at_height(1), Some(b1_id));
    assert_eq!(node.best_block_at_height(2), Some(b2_id));
    assert_eq!(node.best_block_at_height(3), Some(b3_id));
    assert_eq!(node.best_block_at_height(4), Some(b4_id));
    assert_eq!(node.best_block_at_height(5), Some(b5_id));
    assert_eq!(node.best_block_at_height(6), None);

    wallet.sync(&node);
    assert_eq!(wallet.best_height(), 5);
    assert_eq!(wallet.best_hash(), b5_id);


    // assert_eq!(wallet.total_assets_of(Address::Alice), Ok(97));
    // assert_eq!(wallet.total_assets_of(Address::Bob), Ok(3));
    // assert_eq!(wallet.net_worth(), 100);
    
    let mut expected_alice_hash_set = HashSet::new();
    expected_alice_hash_set.insert((coin_id_3, 90 as u64));
    expected_alice_hash_set.insert((coin_id_8, 7 as u64));
    assert_eq!(wallet.all_coins_of(Address::Alice).unwrap(), expected_alice_hash_set);

    let mut expected_bob_hash_set = HashSet::new();
    expected_bob_hash_set.insert((coin_id_9, 3 as u64));
    // assert_eq!(wallet.all_coins_of(Address::Bob), Ok(expected_bob_hash_set));


    assert_eq!(wallet.coin_details(&coin_id_0), Err(WalletError::UnknownCoin));
    assert_eq!(wallet.coin_details(&coin_id_1), Err(WalletError::UnknownCoin));
    assert_eq!(wallet.coin_details(&coin_id_2), Err(WalletError::UnknownCoin));
    assert_eq!(wallet.coin_details(&coin_id_3), Ok(coin_3.clone()));
    assert_eq!(wallet.coin_details(&coin_id_4), Err(WalletError::UnknownCoin));
    assert_eq!(wallet.coin_details(&coin_id_5), Err(WalletError::UnknownCoin));
    assert_eq!(wallet.coin_details(&coin_id_6), Err(WalletError::UnknownCoin));
    assert_eq!(wallet.coin_details(&coin_id_7), Err(WalletError::UnknownCoin));
    assert_eq!(wallet.coin_details(&coin_id_8), Ok(coin_8));
    assert_eq!(wallet.coin_details(&coin_id_9), Ok(coin_9));
}



#[test]
fn reports_correct_ancestors_even_after_reorg_in_the_middle_with_atomic_and_reor_again_to_previous() {
    let mut node = MockNode::new();
    let b1_id = node.add_block_as_best(Block::genesis().id(), vec![]); // B1 is EMPTY

    let coin_0 = Coin {
        value: 100,
        owner: Address::Alice,
    };
    let tx_mint = Transaction {
        inputs: vec![],
        outputs: vec![coin_0.clone()],
    };
    let coin_id_0 = tx_mint.coin_id(2, 0);

    let b2_id = node.add_block_as_best(b1_id, vec![tx_mint]); //B2 WITH MINT TX

    let coin_1 = Coin {
        value: 4,
        owner: Address::Bob,
    };
    let coin_2 = Coin {
        value: 6,
        owner: Address::Bob,
    };
    let coin_3 = Coin {
        value: 90,
        owner: Address::Alice,
    };
    let tx_alice_bob_0 = Transaction {
        inputs: vec![Input {
            coin_id: coin_id_0,
            signature: Signature::Invalid,
        }],
        outputs: vec![coin_1.clone(), coin_2.clone(), coin_3.clone().clone()],
    };

    let coin_id_1 = tx_alice_bob_0.coin_id(3, 0);
    let coin_id_2 = tx_alice_bob_0.coin_id(3, 1);
    let coin_id_3 = tx_alice_bob_0.coin_id(3, 2);

    let b3_id = node.add_block_as_best(b2_id, vec![tx_alice_bob_0.clone()]); // B3 WITH TXS

    let mut wallet = wallet_with_alice_and_bob();
    wallet.sync(&node);
    assert_eq!(wallet.best_height(), 3);
    assert_eq!(wallet.best_hash(), b3_id);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(90));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(10));
    assert_eq!(wallet.net_worth(), 100);

    let mut expected_alice_hash_set = HashSet::new();
    expected_alice_hash_set.insert((coin_id_3, 90 as u64));
    assert_eq!(
        wallet.all_coins_of(Address::Alice).unwrap(),
        expected_alice_hash_set
    );

    let mut expected_bob_hash_set = HashSet::new();
    expected_bob_hash_set.insert((coin_id_1, 4 as u64));
    expected_bob_hash_set.insert((coin_id_2, 6 as u64));
    assert_eq!(wallet.all_coins_of(Address::Bob), Ok(expected_bob_hash_set));

    assert_eq!(
        wallet.coin_details(&coin_id_0),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(wallet.coin_details(&coin_id_1), Ok(coin_1));
    assert_eq!(wallet.coin_details(&coin_id_2), Ok(coin_2.clone()));
    assert_eq!(wallet.coin_details(&coin_id_3), Ok(coin_3.clone()));

    // Now the old blcoks that will be discarted
    let old_b4_id = node.add_block_as_best(b3_id, vec![]); //bob 9, alice 91
    let old_b5_id = node.add_block_as_best(old_b4_id, vec![marker_tx(0123)]);

    let coin_4 = Coin {
        value: 1,
        owner: Address::Alice,
    };
    let coin_5 = Coin {
        value: 3,
        owner: Address::Bob,
    };
    let tx_alice_bob_1 = Transaction {
        inputs: vec![Input {
            coin_id: coin_id_1,
            signature: Signature::Invalid,
        }],
        outputs: vec![coin_4.clone(), coin_5.clone()],
    };

    let coin_id_4 = tx_alice_bob_1.coin_id(6, 0);
    let coin_id_5 = tx_alice_bob_1.coin_id(6, 1);

    let coin_6 = Coin {
        value: 73,
        owner: Address::Alice,
    };
    let coin_7 = Coin {
        value: 20,
        owner: Address::Bob,
    };
    let tx_alice_bob_2 = Transaction {
        inputs: vec![
            Input {
                coin_id: coin_id_3,
                signature: Signature::Invalid,
            },
            Input {
                coin_id: coin_id_5,
                signature: Signature::Invalid,
            },
        ],
        outputs: vec![coin_6.clone(), coin_7.clone()],
    };
    let coin_id_6 = tx_alice_bob_2.coin_id(6, 0);
    let coin_id_7 = tx_alice_bob_2.coin_id(6, 1);
    let old_b6_id = node.add_block_as_best(old_b5_id, vec![tx_alice_bob_1, tx_alice_bob_2]); //bob 29, alice 713
                                                                                             // assert_eq!(node.best_block, old_b6_id);
    assert_eq!(node.best_block_at_height(0), Some(Block::genesis().id()));
    assert_eq!(node.best_block_at_height(1), Some(b1_id));
    assert_eq!(node.best_block_at_height(2), Some(b2_id));
    assert_eq!(node.best_block_at_height(3), Some(b3_id));
    assert_eq!(node.best_block_at_height(4), Some(old_b4_id));
    assert_eq!(node.best_block_at_height(5), Some(old_b5_id));
    assert_eq!(node.best_block_at_height(6), Some(old_b6_id));
    assert_eq!(node.best_block_at_height(7), None);

    wallet.sync(&node);
    assert_eq!(wallet.best_height(), 6);
    assert_eq!(wallet.best_hash(), old_b6_id);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(74));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(26));
    assert_eq!(wallet.net_worth(), 100);

    let mut expected_alice_hash_set = HashSet::new();
    expected_alice_hash_set.insert((coin_id_6, 73 as u64));
    expected_alice_hash_set.insert((coin_id_4, 1 as u64));
    assert_eq!(
        wallet.all_coins_of(Address::Alice).unwrap(),
        expected_alice_hash_set
    );

    let mut expected_bob_hash_set = HashSet::new();
    expected_bob_hash_set.insert((coin_id_7, 20 as u64));
    expected_bob_hash_set.insert((coin_id_2, 6 as u64));
    // expected_bob_hash_set.insert((coin_id_5, 3 as u64));
    assert_eq!(wallet.all_coins_of(Address::Bob), Ok(expected_bob_hash_set));

    assert_eq!(
        wallet.coin_details(&coin_id_0),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(
        wallet.coin_details(&coin_id_1),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(wallet.coin_details(&coin_id_2), Ok(coin_2.clone()));
    assert_eq!(
        wallet.coin_details(&coin_id_3),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(wallet.coin_details(&coin_id_4), Ok(coin_4.clone()));
    assert_eq!(
        wallet.coin_details(&coin_id_5),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(wallet.coin_details(&coin_id_6), Ok(coin_6.clone()));
    assert_eq!(wallet.coin_details(&coin_id_7), Ok(coin_7.clone()));

    let coin_8 = Coin {
        value: 7,
        owner: Address::Alice,
    };
    let coin_9 = Coin {
        value: 3,
        owner: Address::Bob,
    };

    let tx_alice_bob_3 = Transaction {
        inputs: vec![
            Input {
                coin_id: coin_id_1,
                signature: Signature::Invalid,
            },
            Input {
                coin_id: coin_id_2,
                signature: Signature::Invalid,
            },
        ],
        outputs: vec![coin_8.clone(), coin_9.clone()],
    };
    let coin_id_8 = tx_alice_bob_3.coin_id(4, 0);
    let coin_id_9 = tx_alice_bob_3.coin_id(4, 1);
    let b4_id = node.add_block_as_best(b3_id, vec![tx_alice_bob_3]); //bob 29, alice 71

    let b5_id = node.add_block_as_best(b4_id, vec![marker_tx(1011)]);

    // assert_eq!(node.best_block, b5_id);
    assert_eq!(node.best_block_at_height(0), Some(Block::genesis().id()));
    assert_eq!(node.best_block_at_height(1), Some(b1_id));
    assert_eq!(node.best_block_at_height(2), Some(b2_id));
    assert_eq!(node.best_block_at_height(3), Some(b3_id));
    assert_eq!(node.best_block_at_height(4), Some(b4_id));
    assert_eq!(node.best_block_at_height(5), Some(b5_id));
    assert_eq!(node.best_block_at_height(6), None);

    wallet.sync(&node);
    assert_eq!(wallet.best_height(), 5);
    assert_eq!(wallet.best_hash(), b5_id);

    // assert_eq!(wallet.total_assets_of(Address::Alice), Ok(97));
    // assert_eq!(wallet.total_assets_of(Address::Bob), Ok(3));
    // assert_eq!(wallet.net_worth(), 100);

    let mut expected_alice_hash_set = HashSet::new();
    expected_alice_hash_set.insert((coin_id_3, 90 as u64));
    expected_alice_hash_set.insert((coin_id_8, 7 as u64));
    assert_eq!(
        wallet.all_coins_of(Address::Alice).unwrap(),
        expected_alice_hash_set
    );

    let mut expected_bob_hash_set = HashSet::new();
    expected_bob_hash_set.insert((coin_id_9, 3 as u64));
    // assert_eq!(wallet.all_coins_of(Address::Bob), Ok(expected_bob_hash_set));

    assert_eq!(
        wallet.coin_details(&coin_id_0),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(
        wallet.coin_details(&coin_id_1),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(
        wallet.coin_details(&coin_id_2),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(wallet.coin_details(&coin_id_3), Ok(coin_3.clone()));
    assert_eq!(
        wallet.coin_details(&coin_id_4),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(
        wallet.coin_details(&coin_id_5),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(
        wallet.coin_details(&coin_id_6),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(
        wallet.coin_details(&coin_id_7),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(wallet.coin_details(&coin_id_8), Ok(coin_8));
    assert_eq!(wallet.coin_details(&coin_id_9), Ok(coin_9));

    let coin_10 = Coin {
        value: 20,
        owner: Address::Charlie,
    };
    
    let tx_alice_charlie = Transaction {
        inputs: vec![
            Input {
                coin_id: coin_id_7,
                signature: Signature::Invalid,
            }
        ],
        outputs: vec![coin_10.clone()],
    };

    let coin_id_10 = tx_alice_charlie.coin_id(10, 0);



    let b7 = node.add_block_as_best(old_b6_id, vec![tx_alice_charlie]);

    assert_eq!(node.best_block_at_height(0), Some(Block::genesis().id()));
    assert_eq!(node.best_block_at_height(1), Some(b1_id));
    assert_eq!(node.best_block_at_height(2), Some(b2_id));
    assert_eq!(node.best_block_at_height(3), Some(b3_id));
    assert_eq!(node.best_block_at_height(4), Some(old_b4_id));
    assert_eq!(node.best_block_at_height(5), Some(old_b5_id));
    assert_eq!(node.best_block_at_height(6), Some(old_b6_id));
    assert_eq!(node.best_block_at_height(7), Some(b7));
    assert_eq!(node.best_block_at_height(8), None);

    wallet.sync(&node);
    assert_eq!(wallet.best_height(), 7);
    assert_eq!(wallet.best_hash(), b7);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(74));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(6));
    assert_eq!(wallet.net_worth(), 80);

    let mut expected_alice_hash_set = HashSet::new();
    expected_alice_hash_set.insert((coin_id_6, 73 as u64));
    expected_alice_hash_set.insert((coin_id_4, 1 as u64));
    assert_eq!(
        wallet.all_coins_of(Address::Alice).unwrap(),
        expected_alice_hash_set
    );

    let mut expected_bob_hash_set = HashSet::new();
    // expected_bob_hash_set.insert((coin_id_7, 20 as u64));
    expected_bob_hash_set.insert((coin_id_2, 6 as u64));
    // expected_bob_hash_set.insert((coin_id_5, 3 as u64));
    assert_eq!(wallet.all_coins_of(Address::Bob), Ok(expected_bob_hash_set));

    assert_eq!(
        wallet.coin_details(&coin_id_0),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(
        wallet.coin_details(&coin_id_1),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(wallet.coin_details(&coin_id_2), Ok(coin_2));
    assert_eq!(
        wallet.coin_details(&coin_id_3),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(wallet.coin_details(&coin_id_4), Ok(coin_4));
    assert_eq!(
        wallet.coin_details(&coin_id_5),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(wallet.coin_details(&coin_id_6), Ok(coin_6));
    assert_eq!(wallet.coin_details(&coin_id_7), Err(WalletError::UnknownCoin));
    assert_eq!(wallet.coin_details(&coin_id_8), Err(WalletError::UnknownCoin));
    assert_eq!(wallet.coin_details(&coin_id_9), Err(WalletError::UnknownCoin));
    assert_eq!(wallet.coin_details(&coin_id_10), Err(WalletError::UnknownCoin));
    
}