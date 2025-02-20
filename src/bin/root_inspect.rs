use std::{path::Path};
use anyroot::anyblox::*;
use std::env;
 // number parsing
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
    let path = Path::new(filename);
    let file = std::fs::File::open(path).unwrap();
    let mmap = unsafe { Mmap::map(&file).unwrap() };

    // print branch data itself
    let mut state: Option<DecoderState> = None;
    loop {
        println!("pick some space-separated column ids to print or 'exit' to exit");
        println!("> ");
        // read name from stdin
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if input.trim() == "exit" {
            break;
        }

        // split input into column  indices
        let column_mask = input.trim()
                               .split_whitespace()
                               .map(|s| s.parse::<usize>().unwrap())
                               .fold(0u64, |acc, x| acc | 1 << (63 - x));
        println!("parsed column mask: {:b}", column_mask);

        println!("enter start tuple: " );
        input.clear();
        std::io::stdin().read_line(&mut input)?;
        let start = input.trim().parse::<i32>().unwrap();
        println!("parsed start: {}", start);

        println!("enter count: " );
        input.clear();
        std::io::stdin().read_line(&mut input)?;
        let count = input.trim().parse::<i32>().unwrap();

        let batch = decode_batch_internal(&mmap, start, count, &mut state, column_mask);
        println!("decoded batch: {:?}", batch);
        println!("{} rows / {} cols: ", batch.num_rows(), batch.num_columns());
    }
    return Ok(());
}

// dummy main for wasm
#[cfg(target_arch = "wasm32")]
fn main() {}
