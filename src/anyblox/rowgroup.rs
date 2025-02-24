use std::fmt::{Formatter, Debug};
use crate::core::
    types::{Tid}
;
use crate::tree_reader::{Tree, Container, basket_header};
use crate::anyblox::ColumnProjection;

use aligned_vec::AVec;
use failure::Error;
use nom::IResult;

/// row groups are implicit in file format, we need to 'find' them
/// by finding alignment points between containers (= column chunks)
/// of the tree branches.
// #[derive(Debug)]
pub struct RowGroup {
    pub start_tid: Tid,
    pub count: Tid,
    pub containers: Vec<Vec<(u32, u32)>>
}

impl Debug for RowGroup {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RowGroup")
            .field("start_tid", &self.start_tid)
            .field("count", &self.count)
            .field("container_min_max", &(self.containers.iter().fold((usize::MAX, 0usize), |acc, c| (acc.0.min(c.len()), acc.1.max(c.len())))))
            .field("containers[[..2]..2]", &self.containers.iter().take(2).map(|c| c.iter().take(2).collect::<Vec<_>>()).collect::<Vec<_>>())
            .finish()
    }
}

pub struct RowGroupDecodeCursor {
    pub global_col_idx: usize,
    pub projected_col_idx: usize,
    pub byte_count: usize
}

impl RowGroup {

    pub fn container_to_offsets(c: &Container) -> (u32, u32) {
       match c {
           Container::InMemory(_) => panic!("not implemented"),
           Container::OnDisk(_src, start, len) => (*start as u32, *len as u32)
       }
    }

    pub fn end_tid(&self) -> Tid {
        self.start_tid + self.count
    }

    pub fn find_rowgroups(t: &Tree) -> Vec<RowGroup> {
        let branches = &t.fbranches;
        let bcnt = t.branch_count();
        // result
        let mut rowgroups: Vec<RowGroup> = Vec::new();
        let mut container_ids = vec![0usize; bcnt];
        let bundle_cur_containers = |ids: &mut Vec<usize>| {
            (0..bcnt).map(|idx| {
                let off = ids[idx]; ids[idx] += 1;
                vec![Self::container_to_offsets(&branches[idx].containers()[off]); 1]
            }).collect::<Vec<Vec<(u32, u32)>>>()
        };
        let max_tid = t.entries() as Tid;
        let mut bundle_rowgroup = |tid_end: Tid, current: RowGroup, ids: &mut Vec<usize>| {
            let mut res = RowGroup{start_tid: current.start_tid, count: tid_end - current.start_tid, containers: current.containers };
            if tid_end ==  max_tid { // collect rest of containers
              (0..bcnt).for_each(|idx| {
                  let missing_range = ids[idx]..(&branches[idx]).containers().len();
                  missing_range.for_each(|off| {
                      res.containers[idx].push(Self::container_to_offsets(&branches[idx].containers()[off]));
                  });
              });
            }
            rowgroups.push(res);
            println!("found rowgroup: {:?}", rowgroups.last().unwrap());
            RowGroup{start_tid: tid_end, count: 0, containers: if tid_end == max_tid { Vec::new() } else { bundle_cur_containers(ids) } }
        };
        let mut cur = RowGroup{start_tid: 0, count: 0, containers: bundle_cur_containers(&mut container_ids)};
        while cur.start_tid != max_tid {
            // assert we're not OOB for any branch
            assert!(container_ids.iter().enumerate().all(|(idx, id)| *id < (&branches[idx]).containers().len()));
            let first = branches[0].container_start_indices()[container_ids[0]];
            // check whether all branches are in alignment
            let (largest_tid, is_same) = (0..bcnt).fold((first, true), |(tid, same), idx| {
                let branch = &branches[idx];
                let branch_tid = branch.container_start_indices()[container_ids[idx]];
                (branch_tid.max(tid), same && branch_tid == tid)
            });
            if is_same { // all tids were the same -> row group boundary
                cur = bundle_rowgroup(largest_tid, cur, &mut container_ids);
                // this might also be the end iff the very last containers all have the same size
            } else { // advance the smallest tid(s) if not at row group boundary
                for idx in 0..bcnt {
                    let id = &mut container_ids[idx];
                    let indices = (&branches[idx]).container_start_indices();
                    let branch_tid = indices[*id];
                    if branch_tid < largest_tid {
                        cur.containers[idx].push(Self::container_to_offsets(&branches[idx].containers()[*id]));
                        *id += 1;
                    }
                }
            }
            if container_ids.iter().enumerate().any(|(idx, id)| *id == (&branches[idx]).containers().len()) {
                cur = bundle_rowgroup(max_tid, cur, &mut container_ids);
                assert!(cur.start_tid == max_tid);
                break;
            }
        }
        // sum(rowgroup.#entries) == #file.entries
        assert!(rowgroups.iter().fold(0usize, |acc, rg| acc + rg.count as usize) == t.entries() as usize);
        // sum(rowgroup.#containers) == #branch[0].#containers
        assert!(rowgroups.iter().fold(0usize, |acc, rg| acc + rg.containers[0].len()) == branches[0].containers().len());
        assert!(rowgroups.iter().fold(0usize, |acc, rg| acc + rg.containers[0].len()) == branches[0].containers().len());
        // return result
        rowgroups
    }

    pub fn decode<F, T>(&self, mmap: &[u8], cols: u64, mut init: T, consumer: F) -> T
        where F: Fn(T, RowGroupDecodeCursor, &[u8]) -> T
    {
        let colmask = ColumnProjection::from_u64(cols);
        let allcols = self.containers.len();
        let mut colidx=0;
        // XXX output Vec<u8> aligned to 8 bytes
        let mut output: AVec<u8> = AVec::new(8);
        for colid in 0..allcols {
            if !colmask.contains(colid as u32) {
                continue;
            }
            let meta = &self.containers[colid].iter().map(|(start, len)| {
                let buf = &mmap[*start as usize..(*start + *len) as usize];
                basket_header(buf).unwrap().1
            }).collect::<Vec<_>>();
            let totsize = meta.iter().fold(0usize, |acc, m| acc + m.header.uncomp_len as usize);
            if totsize > output.len() {
                output.resize(totsize, 0);
            }
            let written = meta.iter().fold(0usize, |offset, m| {
                let nbyte = m.decode_into(&mut output[offset..(offset+(m.header.uncomp_len as usize))]);
                offset + nbyte
            });
            assert!(written <= totsize);
            init = consumer(
                init,
                RowGroupDecodeCursor{global_col_idx: colid, projected_col_idx: colidx, byte_count: written},
                &output[..written]
            );
            colidx += 1;
        };
        init
    }
}

pub struct DecompressedRowGroup {
    pub start_tid: Tid,
    pub count: Tid,
    pub data: Vec<Vec<u8>>
}

impl DecompressedRowGroup {
    pub fn new(mmap: &[u8], cols: u64, offsets: &RowGroup) -> Self {
        let mut coldata = vec![Vec::new(); cols.count_ones() as usize];
        coldata = offsets.decode(mmap, cols, coldata, |mut cols, cursor, bytes| {
            cols[cursor.projected_col_idx] = bytes.to_vec();
            cols
        });
        DecompressedRowGroup{
            start_tid: offsets.start_tid,
            count: offsets.count,
            data: coldata
        }
    }

    pub fn parse_col<P, G, T>(&self, col: usize, parser: P, mut consumer: G) -> Result<(), Error>
    where
        P: Fn(&[u8]) -> IResult<&[u8], T>,
        G: FnMut(usize, T) -> (),
    {
        let mut input: &[u8] = &self.data[col].as_slice();
        for idx in 0..self.count {
            let input_: &[u8] = input;
            match parser(input_) { // use nom parsers
                Ok((i, o)) => {
                    consumer(idx as usize, o);
                    input = i;
                },
                Err(e) => {
                    dbg!("error at {} with err {:?}", idx, e);
                    return Err(failure::err_msg("parse error"));
                }
            }
        }
        Ok(())
    }
}
