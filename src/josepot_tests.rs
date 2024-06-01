//! Written by josepot

use bonecoin_core::*;
use std::collections::*;

#[test]
fn use_small_coins_first() {
    let initial_tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![
            Coin {
                value: 100,
                owner: Address::Alice,
            },
            Coin {
                value: 50,
                owner: Address::Bob,
            },
            Coin {
                value: 12,
                owner: Address::Bob,
            },
            Coin {
                value: 500,
                owner: Address::Alice,
            },
            Coin {
                value: 5,
                owner: Address::Bob,
            },
            Coin {
                value: 150,
                owner: Address::Bob,
            },
            Coin {
                value: 3,
                owner: Address::Alice,
            },
        ],
    };

    // we will spend a total amount of 20, so the coins with the values 3, 5 and 12 should be used
    let expected_inputs: HashSet<CoinId> = vec![2 as usize, 4, 6]
        .iter()
        .map(|idx| initial_tx.coin_id(1, *idx))
        .collect();

    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), vec![initial_tx]);

    let mut wallet = Wallet::new(vec![Address::Alice, Address::Bob].into_iter());
    wallet.sync(&node);

    let tx = wallet
        .create_automatic_transaction(Address::Eve, 17, 3)
        .unwrap();

    assert_eq!(
        tx.inputs
            .iter()
            .all(|x| expected_inputs.contains(&x.coin_id))
            && tx.inputs.len() == expected_inputs.len(),
        true
    );
}
