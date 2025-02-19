use std::fmt::{Formatter, Debug};
use crate::core::{
    types::{Tid},
    tkey_header,
    decompress
};
use crate::anyblox::parse::consume_count;
use crate::tree_reader::{Tree, Container, BasketHeader, basket_header};

use bitvec::prelude::*;
use bitvec::view::BitView;
use failure::Error;
use nom::IResult;

/// row groups are implicit in file format, we need to 'find' them
/// by finding alignment points between containers (= column chunks)
/// of the tree branches.
// #[derive(Debug)]
pub struct RowGroup {
    start_tid: Tid,
    count: Tid,
    containers: Vec<Vec<(u32, u32)>>
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

impl RowGroup {

    pub fn container_to_offsets(c: &Container) -> (u32, u32) {
       match c {
           Container::InMemory(_) => panic!("not implemented"),
           Container::OnDisk(_src, start, len) => (*start as u32, *len as u32)
       }
    }

    pub fn find_rowgroups(t: &Tree) -> Vec<RowGroup> {
        let branches = &t.branches();
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
                  (ids[idx]..(&branches[idx]).container_start_indices().len()).for_each(|off| {
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
            let (smallest_tid, is_same) = (0..bcnt).fold((first, true), |(tid, same), idx| {
                let branch = &branches[idx];
                let branch_tid = branch.container_start_indices()[container_ids[idx]];
                (branch_tid.min(tid), same && branch_tid == tid)
            });
            if is_same { // all tids were the same -> row group boundary
                cur = bundle_rowgroup(smallest_tid, cur, &mut container_ids);
                container_ids.iter_mut().for_each(|id| *id += 1);
                // this might also be the end iff the very last containers all have the same size
                if container_ids[0] == (&branches[0]).container_start_indices().len() {
                    dbg!("end of file");
                    // should be the end for all - assert that
                    assert!(container_ids.iter().enumerate().all(|(idx, id)| *id == (&branches[idx]).containers().len()));
                    assert!((cur.start_tid as i64) == t.entries());
                    break;
                }
            } else { // advance the smallest tid(s) if not at row group boundary
                let mut at_end = false;
                for idx in 0..bcnt {
                    let id = &mut container_ids[idx];
                    let indices = (&branches[idx]).container_start_indices();
                    let branch_tid = indices[*id];
                    if branch_tid == smallest_tid {
                        cur.containers[idx].push(Self::container_to_offsets(&branches[idx].containers()[*id]));
                        *id += 1;
                        at_end |= *id == indices.len();
                    }
                }
                if at_end {
                    cur = bundle_rowgroup(max_tid, cur, &mut container_ids);
                }
            }
        }
        // return result
        rowgroups
    }
}

pub struct DecompressedRowGroup {
    start_tid: Tid,
    count: Tid,
    data: Vec<Vec<u8>>
}

impl DecompressedRowGroup {
    pub fn new(mmap: &[u8], cols: u64, offsets: &RowGroup) -> Self {
        let colmask = cols.view_bits::<Msb0>();
        let allcols = offsets.containers.len();
        let mut coldata = vec![Vec::new(); cols.count_ones() as usize];
        for colid in 0..allcols {
            if !colmask[colid] {
                continue;
            }
            let meta = &offsets.containers[colid].iter().map(|(start, len)| {
                let buf = &mmap[*start as usize..(*start + *len) as usize];
                basket_header(buf).unwrap().1
            }).collect::<Vec<_>>();
            let totsize = meta.iter().fold(0usize, |acc, m| acc + m.header.uncomp_len as usize);
            coldata[colid as usize] = vec![0u8; totsize];
            let written = meta.iter().fold(0usize, |offset, m| {
                let nbyte = m.decode_into(&mut coldata[colid as usize][offset..]);
                offset + nbyte
            });
            assert!(written < totsize);
        };
        DecompressedRowGroup{
            start_tid: offsets.start_tid,
            count: offsets.count,
            data: coldata
        }
    }

    pub fn parse_col<F, P, G, T>(&self, col: usize, parser: P, mut consumer: G) -> Result<(), Error>
    where P: Fn(&[u8]) -> IResult<&[u8], T>,
          G: FnMut(usize, T) -> (),
    {
        let mut input: &[u8] = (&self.data[col][..]).clone();
        for idx in 0..self.count {
            let input_: &[u8] = input.clone();
            match parser(input_) { // use nom parsers
                Ok((i, o)) => {
                    consumer(idx as usize, o);
                    input = i;
                },
                Err(e) => {
                    dbg!(e);
                    return Err(failure::err_msg("parse error"));
                }
            }
        }
        Ok(())
    }
}
