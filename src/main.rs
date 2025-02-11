use memmap::Mmap;

// helper macro for reading type T from byte slice in big endian
macro_rules! read_be {
    ($buf: expr, $type:ident, $start:expr, $end:expr) => {
        $type::from_be_bytes($buf[$start..$end].try_into().unwrap())
    };
}

// ROOT file format
// from https://github.com/root-project/root/blob/master/io/io/src/TFile.cxx
// A ROOT file is composed of a header, followed by consecutive data records
// (`TKey` instances) with a well defined format.
//
// The first data record starts at byte fBEGIN (currently set to kBEGIN).
// Bytes 1->kBEGIN contain the file description, when fVersion >= 1000000
// it is a large file (> 2 GB) and the offsets will be 8 bytes long and
// fUnits will be set to 8:
//
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
#[derive(Debug)]
pub struct RootHeader {
    version: u32,
    begin: u32,
    end: u32,
    seek_free: u32,
    nbytes_free: u32,
    nfree: u32,
    nbytes_name: u32,
    units: u8,
    compress: u32,
    seek_info: u32,
    nbytes_info: u32,
    uuid: u128
}

fn read_header(mmap: &[u8]) -> RootHeader {
    // assume file < 4GB -> offsets are 4 bytes
    return RootHeader {
        version     : read_be!(mmap, u32, 4, 8),
        begin       : read_be!(mmap, u32, 8, 12),
        end         : read_be!(mmap, u32, 12, 16),
        seek_free   : read_be!(mmap, u32, 16, 20),
        nbytes_free : read_be!(mmap, u32, 20, 24),
        nfree       : read_be!(mmap, u32, 24, 28),
        nbytes_name : read_be!(mmap, u32, 28, 32),
        units       : read_be!(mmap, u8, 32, 33),
        compress    : read_be!(mmap, u32, 33, 37),
        seek_info   : read_be!(mmap, u32, 37, 41),
        nbytes_info : read_be!(mmap, u32, 41, 45),
        uuid        : read_be!(mmap, u128, 45, 61)
    }
}

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
#[derive(Debug)]
pub struct RootObjHeader {
    nbytes: u32,
    version: u16,
    obj_len: u32,
    datime: u32,
    key_len: u16,
    cycle: u16,
    seek_key: u32,
    seek_pdir: u32,
    class_name: String,
    name: String,
    title: String
}

fn read_obj_header(mmap: &[u8]) -> RootObjHeader {
    // assume file < 4GB -> offsets are 4 bytes
    let c_lname = mmap[26] as usize;
    let o_lname = mmap[27 + c_lname] as usize;
    let t_lname = mmap[27 + c_lname + o_lname + 1] as usize;
    println!("c_lname: {}, o_lname: {}, t_lname: {}", c_lname, o_lname, t_lname);
    return RootObjHeader {
        nbytes     : read_be!(mmap, u32, 0, 4),
        version    : read_be!(mmap, u16, 4, 6),
        obj_len    : read_be!(mmap, u32, 6, 10),
        datime     : read_be!(mmap, u32, 10, 14),
        key_len    : read_be!(mmap, u16, 14, 16),
        cycle      : read_be!(mmap, u16, 16, 18),
        seek_key   : read_be!(mmap, u32, 18, 22),
        seek_pdir  : read_be!(mmap, u32, 22, 26),
        // class_name : String::new(),
        class_name : String::from_utf8(mmap[27..(27+c_lname)].to_vec()).unwrap(),
        // name       : String::new(),
        name       : String::from_utf8(mmap[(27+c_lname)..(27+c_lname+o_lname+1)].to_vec()).unwrap(),
        // title      : String::new()
        title      : String::from_utf8(mmap[(27+c_lname+o_lname+1)..(27+c_lname+o_lname+t_lname+1)].to_vec()).unwrap()
    }

}

fn main() {
    // RootFile::new("../b2hhh.zstd.root");
    let filename = "../b2hhh.zstd.root";
    println!("Opening file: {}", filename);
    let path = std::path::Path::new(filename);
    // mmap the file
    let file = std::fs::File::open(path).unwrap();
    let mmap = unsafe { Mmap::map(&file).unwrap() };
    // read the header according to the spec
    let header = read_header(&mmap);
    println!("{:?}", header);
    // read first object header according to the spec
    let obj_header = read_obj_header(&mmap[header.begin as usize..]);
    println!("{:?}", obj_header);
}
