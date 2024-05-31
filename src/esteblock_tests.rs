// Written by esteblock

use bonecoin_core::*;
use utxo_wallet_assignment::Wallet;

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
