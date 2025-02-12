use crate::anyroot::reader::*;
////////////////////////////////////////////////////////////////////////////////
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
#[allow(dead_code)]
#[derive(Debug)]
pub struct RecordHeader {
    pub nbytes: u32,
    pub version: u16,
    pub obj_len: u32,
    pub datime: u32,
    pub key_len: u16,
    pub cycle: u16,
    pub seek_key: fileptr,
    pub seek_pdir: fileptr,
    pub class_name: String,
    pub name: String,
    pub title: String
}

impl MMapReader for RecordHeader {
    fn read(mmap: &[u8]) -> (Self, usize) {
        // assume file < 4GB -> offsets are 4 bytes
        let c_lname = mmap[26] as usize;
        let o_lname = mmap[27 + c_lname] as usize;
        let t_lname = mmap[27 + c_lname + o_lname + 1] as usize;
        let bytecount = 27+c_lname+o_lname+t_lname+2;
        println!("c_lname: {}, o_lname: {}, t_lname: {}", c_lname, o_lname, t_lname);
        let hdr = RecordHeader {
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
            name       : String::from_utf8(mmap[(27+c_lname+1)..(27+c_lname+o_lname+1)].to_vec()).unwrap(),
            // title      : String::new()
            title      : String::from_utf8(mmap[(27+c_lname+o_lname+2)..(bytecount)].to_vec()).unwrap()
        };
        return (hdr, bytecount)
    }
}

////////////////////////////////////////////////////////////////////////////////
// parsed out of code from inofficial go root implementation
// https://github.com/go-hep/hep/blob/main/groot/rbytes/rbuffer.go#L99 and following

#[allow(dead_code)]
#[derive(Debug)]
pub struct ObjHeader {

}

impl MMapReader for ObjHeader {
    fn read(mmap: &[u8]) -> (Self, usize) {

    }
}
