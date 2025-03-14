//! A convenience wrapper and needed parsers to work with ROOT's
//! `TTree`s. A Tree may be thought of as a table where each row
//! represents a particle collision. Each column may contain one or
//! several elements per collision. This module provides two Iterator
//! structs in order to iterate over these columns (`TBranches` in
//! ROOT lingo).

mod branch;
mod container;
mod leafs;
mod tree;

pub use self::tree::{ttree, Tree};
pub use self::container::{Container, BasketHeader, basket_header};

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::path::PathBuf;
    use crate::core::RootFile;

    #[test]
    fn simple_tree() {
        let path = PathBuf::from("./src/test_data/simple.root");
        let f = RootFile::new(path.as_path()).expect("Failed to open file");
        f.items()[0].as_tree().unwrap();
    }
}
