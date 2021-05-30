fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use std::fs::File;
    use zcash_client_backend::wallet::AccountId;
    use zcash_client_backend::welding_rig::scan_block;
    use zcash_primitives::consensus::Network;
    use zcash_primitives::merkle_tree::CommitmentTree;
    use zcash_primitives::sapling::{Nullifier, SaplingIvk};

    #[test]
    fn test_load_block_json() {
        let file = File::open("data/block.json").unwrap();
        let block: Value = serde_json::from_reader(file).unwrap();
        println!("{}", block);
        let mut tree = CommitmentTree::empty();
        let ivks = Vec::<(&AccountId, &SaplingIvk)>::new();
        let nullifiers = Vec::<(AccountId, Nullifier)>::new();
        scan_block(
            &Network::TestNetwork,
            Default::default(),
            &ivks,
            &nullifiers,
            &mut tree,
            &mut [],
        );
    }
}
