use std::{path::Path};
use anyroot::*;
use anyroot::anyblox::*;
use anyroot::core::Tid;
use std::env;
use nom::number::complete::*; // number parsing
use memmap::Mmap;

// ROOT file format
// from https://github.com/root-project/root/blob/master/io/io/src/TFile.cxx
// Byte Range      | Record Name | Description
// ----------------|-------------|------------
// 1->4            | "root"      | Root file identifier
// 5->8            | fVersion    | File format version
// 9->12           | fBEGIN      | Pointer to first data record
// 13->16 [13->20] | fEND        | Pointer to first free word at the EOF
// 17->20 [21->28] | fSeekFree   | Pointer to FREE data record
// 21->24 [29->32] | fNbytesFree | Number of bytes in FREE data record
// 25->28 [33->36] | nfree       | Number of free data records
// 29->32 [37->40] | fNbytesName | Number of bytes in TNamed at creation time
// 33->33 [41->41] | fUnits      | Number of bytes for file pointers
// 34->37 [42->45] | fCompress   | Compression level and algorithm
// 38->41 [46->53] | fSeekInfo   | Pointer to TStreamerInfo record
// 42->45 [54->57] | fNbytesInfo | Number of bytes in TStreamerInfo record
// 46->63 [58->75] | fUUID       | Universal Unique ID

// The key structure is as follows; if a key is located past the 32 bit file
// limit (> 2 GB) then some fields will be 8 instead of 4 bytes (see parts marked
// with square brackets below):

// Byte Range      | Member Name | Description
// ----------------|-----------|--------------
// 1->4            | Nbytes    | Length of compressed object (in bytes)
// 5->6            | Version   | TKey version identifier
// 7->10           | ObjLen    | Length of uncompressed object
// 11->14          | Datime    | Date and time when object was written to file
// 15->16          | KeyLen    | Length of the key structure (in bytes)
// 17->18          | Cycle     | Cycle of key
// 19->22 [19->26] | SeekKey   | Pointer to record itself (consistency check)
// 23->26 [27->34] | SeekPdir  | Pointer to directory header
// 27->27 [35->35] | lname     | Number of bytes in the class name
// 28->.. [36->..] | ClassName | Object Class Name
// ..->..          | lname     | Number of bytes in the object name
// ..->..          | Name      | lName bytes with the name of the object
// ..->..          | lTitle    | Number of bytes in the object title
// ..->..          | Title     | Title of the object
// ----->          | DATA      | Data bytes associated to the object

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    // RootFile::new("../b2hhh.zstd.root");
    // if arg given, read file from given  name, otherwise default
    let default_file = String::from("../cern.ch:84000.root");
    let filename: &String = if args.len() <= 1 {&default_file} else {&args[1]};
    println!("Opening file: {}", filename);
    let path = Path::new(filename);
    let rf = RootFile::new(path)?;
    println!("File: {:?}", rf);
    let tree = rf.items()[0].as_tree()?;
    let branches = tree.main_branches();

    println!("tree entries: {}", tree.entries());
    println!("searching for row groups...");
    let rgs = anyblox::RowGroup::find_rowgroups(&tree);
    println!("found {} rowgroups in tree", rgs.len());

    let file = std::fs::File::open(path).unwrap();
    let mmap = unsafe { Mmap::map(&file).unwrap() };

    // break before printing lots
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    for b in branches.iter() {
        println!("{}: {:?}", b.name(), b.element_types());
    }
    // print branch data itself
    loop {
        println!("pick one of {} f64 branch (column) to print or 'exit' to exit", branches.len());
        println!("> ");
        // read name from stdin
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if input.trim() == "exit" {
            break;
        }
        let idx = tree.branch_index(&input.trim()).unwrap();
        let decomp = DecompressedRowGroup::new(&mmap[..], u64::MAX, &rgs[0]);

        decomp.parse_col(idx, |i| be_f64(i), |idx: usize, val: f64| {
            println!("{}: {}", idx, val);
        })?;
        break;
        match tree.branch_by_name(&input.trim()) {
            Ok(branch) => {
                branch.iterate_fixed_size(|i| be_f64(i), |item, idx| {
                    println!("item: {:?}", item);
                    return idx < 10;
                });
                // println!("branch {:?}", branch);
                // println!("branch with {} containers and {} items overall ", branch.containers().len(), branch.entries());
                // let container_lengths = branch.container_start_indices().iter().scan(0, |acc, &x| {
                //     let prev = *acc;
                //     *acc = x;
                //     return Some(x - prev);
                // }).collect::<Vec<Tid>>();
                // println!("container start idx: {:?}", branch.container_start_indices());
                // println!("container lengths: {:?}", container_lengths);
                // continue;

                // let mut cnt_sum : usize = 0;
                // let mut cnt_min : usize = usize::MAX;
                // let mut cnt_max : usize = 0;
                // for container in branch.containers() {
                //     let (cnt, _data) = container.clone().raw_data().unwrap();
                //     let c = cnt as usize;
                //     cnt_sum += c;
                //     cnt_min = cnt_min.min(c);
                //     cnt_max = cnt_max.max(c);
                // }
                // println!("container stats: sum: {}, min: {}, max: {}, avg {}", cnt_sum, cnt_min, cnt_max, cnt_sum / branch.containers().len());

            },
            Err(e) => {
                println!("Branch not found: {}", e);
            }
        }
    }
    return Ok(());
}

// dummy main for wasm
#[cfg(target_arch = "wasm32")]
fn main() {}
