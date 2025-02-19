use crate::core::{
    RootFile,
    Directory,
    TKeyHeader,
    types::{Tid}
};
use crate::tree_reader::{Tree};
use crate::anyblox::RowGroup;

use std::cmp::Ordering;
use bitvec::prelude::*;
use bitvec::view::BitView;

// decode_batch params
// - i32 data, the pointer to the place in Decoder’s linear mem-
// ory where the encoded data starts;
// - i32 data_length, the length in bytes of the encoded data;
// - i32 start_tuple, the ID of the first tuple to decode;
// - i32 tuple_count, the number of tuples to decode;
// - i32 state, the pointer to the place in Wasm’s linear mem-
// ory where the job state is stored;
// - i64 projection_mask, the bitmask specifying a column
// projection.

// when first calling decode_batch, we need to decode & 'binary search' through the data,
// but we get to keep some state for future calls.
//
// reminder: high-level file structure
// file -[1:N]-> TTree -[1:N]-> TBranch(=Column) -[1:N]-> (Subbranches +) TLeaf -[1:N]-> Value
//
// simplifying assumptions( observations in practice)
// - a file usually has only one TTree
// - a TBranch usually has only one TLeaf (= Element Type)
// - a TBranch usually has no subbranches
// - a TLeaf usually only holds one type of value
//
// Under these assumptions:
// - a TBranch is equivalent to a column
// - a TTree is equivalent to a table
// - a File only holds one table
//
// Physical TBranch Layout:
// - a TBranch has multiple containers, each with (in practice) one 'basket'
// - these are 'column chunks' in relational lingo.
//
// Compression:
// AFAIK, ROOT may compress each 'object' (TTree, TBranch, Container) *separately*,
// .e.g., a LZ4-compressed tree can contain XZ-compressed Containers etc.,
// as defined by the 'compression' header of TKey objects (see, for example, `tbasket2vec` function)

#[derive(Debug)]
struct DecoderFileState {
    // / file directory
    // dir: Directory,
    // /// tkey pointing to list of keys in file
    // keylist: TKeyHeader,
    // / ttree tuple start for binary search
    file: RootFile,
    tuples: Tid,
    // ttree_end_tids: Vec<Tid>,
    // XXX how to ensure allocation is in state page?
    rowgroups: Vec<RowGroup>
}

impl DecoderFileState {
    // /// find index of tree containing tuple id
    // pub fn find_tree_containing_tid(&self, tuple: Tid) -> Option<usize> {
    //     // lower bound
    //     self.ttree_end_tids.binary_search_by(|element| match element.cmp(&tuple) {
    //         Ordering::Equal => Ordering::Greater,
    //         ord => ord,
    //     }).ok()
    // }

    pub fn tree_at(&self, uid: u32) -> Tree {
        self.file.items()[uid as usize].as_tree().unwrap()
    }

    pub fn new(data: &'static [u8]) -> Self {
        let file = match RootFile::new(data) {
            Ok(f) => f,
            _ => panic!("failed to parse root file")
        };
        // only one ttree supported for now
        assert!(file.items().len() == 1);
        // let items = file.items();
        // let mut tids: Vec<Tid> = items.iter().map(|item| {item.as_tree().unwrap().entries() as Tid }).collect();
        // let mut cumsum = 0;
        // for x in &mut tids {
        //     cumsum += *x;
        //     *x = cumsum;
        // }
        let tree = file.items().first().unwrap().as_tree().unwrap();
        Self {
            file,
            tuples: tree.entries() as Tid,
            rowgroups: RowGroup::find_rowgroups(&tree),
            // ttree_end_tids: tids
        }
    }
}

#[derive(Debug)]
struct ColumnCache {
    prev_basket_id: u32,
    /// XXX how to ensure allocation is in state page?
    prev_basket_data: Vec<u8>
}

// issue: with ~3k containers and different sizes per tbranch,
// there will be multiple kb of metadata
// new plan:
// - parse all the container headers in all tbranches
// - save file offsets, tuple counts to metadata header
// - decode_batch can reference that, doesn't need to keep it in thread-local state
#[derive(Debug)]
struct DecoderCache {
    // prev_ttree_id: u32,
    prev_columns: u64,
    prev_tid_end: Tid,
    /// XXX how to ensure allocation is in state page?
    prev_columns_data: Vec<ColumnCache>
}

impl DecoderCache {
    pub fn new(global: &DecoderFileState, start_tuple: Tid, tuple_count: Tid, columns: u64) -> Self {
        let ttree_id = 0; // only one ttree supported for now
        // let ttree_id = match global.find_tree_containing_tid(start_tuple) {
        //     Some(x) => x as u32,
        //     None => panic!("failed to find tree containing tuple " )
        // };

        let tree = global.tree_at(ttree_id);
        let bitmask = Self::col_bitmask(&columns);
        let colcount = columns.count_ones() as usize;
        let mut cols : Vec<ColumnCache> = Vec::with_capacity(colcount);
        for (col, branch) in tree.main_branches().iter().enumerate() {
            if !bitmask[col] {
                continue;
            }
            assert!(branch.entries() == tree.entries());
            // branch.l
            cols.push(ColumnCache{prev_basket_id: 0, prev_basket_data: Vec::new()});
        }

        DecoderCache{prev_columns: columns, prev_tid_end: start_tuple + tuple_count, prev_columns_data: cols}
    }

    pub fn col_bitmask<'a>(columns: &'a u64) -> &'a BitSlice<u64, Msb0> {
        // XXX how is the projection mask encoded? Msb or lsb == col0?
        columns.view_bits::<Msb0>()
    }
}

#[derive(Debug)]
struct DecoderState {
    /// file state that is unchanging across all calls in this file
    file: DecoderFileState,
    /// 'cache' state that might change
    cache: DecoderCache
}

impl DecoderState {
    fn new(data: &'static [u8], start_tuple: Tid, tuple_count: Tid, columns: u64) -> Self {
        let file = DecoderFileState::new(data);
        let cache = DecoderCache::new(&file, start_tuple, tuple_count, columns);
        DecoderState{file, cache}
    }
}

fn decode_batch_internal<'a>(data: &'a [u8], start_tuple: Tid, tuple_count: Tid, state: &mut Option<DecoderState>, columns: u64) {
    if state.is_none() {
        // ...well...
        let static_data: &'static [u8] = unsafe { std::mem::transmute(data) };
        state.replace(DecoderState::new(static_data, start_tuple, tuple_count, columns));
    }
}
