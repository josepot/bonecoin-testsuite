use bonecoin_core::*;
use std::collections::*;
use utxo_wallet_assignment::Wallet;

// #[test]
// fn total_assets_of_should_not_return_no_owned_address() {
//     // https://discord.com/channels/1219966585582653471/1246066143907811368/1246112529189568555
//     let wallet = Wallet::new(vec![].into_iter());

//     assert_eq!(
//         wallet.total_assets_of(Address::Bob),
//         Err(WalletError::ForeignAddress)
//     );
// }

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

    let block_1 = node.add_block_as_best(Block::genesis().id(), vec![mint_tx.clone(), tx2.clone(), tx3]);
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 1);
    assert_eq!(wallet.best_hash(), block_1);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(0));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(0));
    assert_eq!(wallet.net_worth(), 0);
}
