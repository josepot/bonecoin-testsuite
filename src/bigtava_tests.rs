//! Written by bigtava

use std::collections::*;
use bonecoin_core::*;
use utxo_wallet_assignment::Wallet;

fn wallet_with_alice() -> Wallet {
    Wallet::new(vec![Address::Alice].into_iter())
}

#[test]
fn test_reorgs_with_utxos_in_chain_history() {
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice();

    let coin_1 = Coin { value: 50, owner: Address::Alice };
    let coin_2 = Coin { value: 100, owner: Address::Alice };

    let tx_1 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin_1.clone()],
    };
    let tx_2 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin_2.clone()],
    };

    // Old chain
    let b1_id = node.add_block(Block::genesis().id(), vec![]);
    let b2_id = node.add_block(b1_id, vec![]);
    let b3_id = node.add_block(b2_id, vec![tx_1.clone()]);
    let old_b4_id = node.add_block(b3_id, vec![]);
    let old_b5_id = node.add_block(old_b4_id, vec![]);
    let old_b6_id = node.add_block_as_best(old_b5_id, vec![tx_2.clone()]);
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 6);
    assert_eq!(wallet.best_hash(), old_b6_id);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(150));
    assert_eq!(wallet.net_worth(), 150);

    // New chain
    let new_coin = Coin { value: 200, owner: Address::Alice };
    let tx_new = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![new_coin.clone()],
    };

    let new_b4_id = node.add_block_as_best(b3_id, vec![tx_new]);
    let new_b5_id = node.add_block_as_best(new_b4_id, vec![]);
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 5);
    assert_eq!(wallet.best_hash(), new_b5_id);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(250));
    assert_eq!(wallet.net_worth(), 250);
}