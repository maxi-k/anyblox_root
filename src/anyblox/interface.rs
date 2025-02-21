use crate::{
    anyblox::{branches_to_arrow_schema, rowgroup_to_record_batch, RowGroup},
    core::{types::Tid, RootFile}
};

use std::{cmp::Ordering, sync::Arc};

use arrow::record_batch::RecordBatch;

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
    #[allow(dead_code)]
    tuples: Tid,
    rowgroups: Vec<RowGroup>,
    columns: Vec<(String, String)> // name/type pairs
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
    pub fn find_rowgroup_containing_tid(&self, tuple: Tid) -> usize {
        self.rowgroups.binary_search_by(|rg| match (rg.end_tid()-1).cmp(&tuple) {
            Ordering::Equal => Ordering::Greater, // lower bound
           ord => ord
        }).unwrap_err()
    }

    pub fn new(data: &'static [u8]) -> Self {
        let file = match RootFile::new(data) {
            Ok(f) => f,
            _ => panic!("failed to parse root file")
        };
        // only one ttree supported for now
        assert!(file.items().len() == 1);
        let tree = file.items().first().unwrap().as_tree().unwrap();
        Self {
            tuples: tree.entries() as Tid,
            rowgroups: RowGroup::find_rowgroups(&tree),
            columns: tree.main_branch_names_and_types(),
            // ttree_end_tids: tids
        }
    }
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
    prev_columns: u64, // did the projection bitmask change?
    batch_tid_start: Tid, // last tid produced for last request
    batch_size: Tid,
    batch: RecordBatch
}

impl DecoderCache {
    pub fn new(data: &[u8], global: &DecoderFileState, start_tuple: Tid, _tuple_count: Tid, columns: u64) -> Self {
        let rg = global.find_rowgroup_containing_tid(start_tuple);
        let group = &global.rowgroups[rg];
        let schema = Arc::new(branches_to_arrow_schema(global.columns.as_slice(), columns));
        DecoderCache{
            prev_columns: columns,
            batch_tid_start: group.start_tid,
            batch_size: group.count,
            batch: rowgroup_to_record_batch(data, columns, group, schema)
        }
    }

    /// potentially invalidates current cache, returns the record batch slice we can read
    pub fn invalidate(&mut self, data: &[u8], global: &DecoderFileState, start_tuple: Tid, tuple_count: Tid, columns: u64) -> RecordBatch {
        let in_range = start_tuple >= self.batch_tid_start && start_tuple < self.batch_tid_end();
            // projection mask changed or cur row group does not have correct range
        if columns != self.prev_columns || !in_range {
            *self = DecoderCache::new(data, global, start_tuple, tuple_count, columns);
        }
        let start = start_tuple - self.batch_tid_start;
        // (XXX make sure that this is does not copy the columns)
        // https://docs.rs/arrow/latest/arrow/array/struct.RecordBatch.html#method.slice
        self.batch.slice(start as usize, tuple_count.min(self.batch_size - start) as usize)
    }

    fn batch_tid_end(&self) -> Tid {
        self.batch_tid_start + self.batch_size
    }
}

#[derive(Debug)]
pub struct DecoderState {
    /// file state that is unchanging across all calls in this file
    file: DecoderFileState,
    /// 'cache' state that might change
    cache: DecoderCache
}

impl DecoderState {
    fn new(data: &'static [u8], start_tuple: Tid, tuple_count: Tid, columns: u64) -> Self {
        let file = DecoderFileState::new(data);
        let cache = DecoderCache::new(data, &file, start_tuple, tuple_count, columns);
        DecoderState{file, cache}
    }
}

pub fn decode_batch_internal<'a>(data: &'a [u8], start_tuple: Tid, tuple_count: Tid, state: &mut Option<DecoderState>, columns: u64) -> RecordBatch {
    let s: &mut DecoderState = state.get_or_insert_with(|| {
        let static_data: &'static [u8] = unsafe { std::mem::transmute(data) };
        DecoderState::new(static_data, start_tuple, tuple_count, columns)
    });
    s.cache.invalidate(data, &s.file, start_tuple, tuple_count, columns)
}
