use crate::anyroot::reader::*;
use crate::anyroot::header::*;

// adapted from
// - root ttree      : https://github.com/root-project/root/blob/master/tree/tree/inc/TTree.h
//                   : https://github.com/root-project/root/blob/master/tree/tree/src/TTree.cxx
// - root-io         : https://github.com/cbourjau/alice-rs/tree/master/root-io
// - go ttree writer : https://github.com/go-hep/hep/blob/main/groot/rtree/writer.go
#[derive(Debug)]
#[allow(dead_code)]
pub struct TreeObj {
    pub encoding: ObjHeaderEncoding,
    pub version: u16,               // Version of the read layout
    pub name: ObjName,              // The basis for a named object (name, title)
    pub entries: i64,               // Number of entries
    pub totbytes: i64,              // Total number of bytes in all branches before compression
    pub zipbytes: i64,              // Total number of bytes in all branches after compression
    pub savedbytes: i64,            // Number of autosaved bytes
    // pub flushedbytes: Option<i64>,  // Number of autoflushed bytes
    // pub weight: f64,                // Tree weight (see TTree::SetWeight)
    // pub timerinterval: i32,         // Timer interval in milliseconds
    // pub scanfield: i32,             // Number of runs before prompting in Scan
    // pub update: i32,                // Update frequency for EntryLoop
    // pub maxentries: i64,            // Maximum number of entries in case of circular buffers
    // pub maxentryloop: i64,          // Maximum number of entries to process
    // pub estimate: i64,              // Number of entries to estimate histogram limits
    // pub branches: Vec<TBranch>,     // List of Branches
    // pub leaves: Vec<TLeaf>,         // Direct pointers to individual branch leaves
    // pub aliases: Option<Vec<u8>>,   // List of aliases for expressions based on the tree branches.
    // pub indexvalues: Vec<f64>,      // Sorted index values
    // pub index: Vec<i32>,            // Index of sorted values
    // pub treeindex: Option<fileptr>, // Pointer to the tree Index (if any)
    // pub friends: Option<fileptr>,   // pointer to list of friend elements
    // pub userinfo: Option<fileptr>,  // pointer to a list of user objects associated to this Tree
    // pub branchref: Option<fileptr>, // Branch supporting the TRefTable (if any)
}

impl TreeObj {

}

impl MMapReader for TreeObj {
    fn read(src: &[u8]) -> (Self, usize) {
        let (encoding, nb) = ObjHeaderEncoding::read(&src[0..]);
        let fsrc = &src[nb..];
        let ver = read_be!(src, u16, 0, 2);
        println!("encoding: {:?}, ver {:?}, ({})", encoding, ver, nb);
        let (name, fsrc) = ObjName::take(&fsrc[0..]);
        let tree = TreeObj {
            encoding: encoding,
            version: ver,
            name: name,
            entries: read_be!(fsrc, i64, 0, 8),
            totbytes: read_be!(fsrc, i64, 8, 16),
            zipbytes: read_be!(fsrc, i64, 16, 24),
            savedbytes: read_be!(fsrc, i64, 24, 32),
        };
        return (tree, nb + 32);
    }
}
