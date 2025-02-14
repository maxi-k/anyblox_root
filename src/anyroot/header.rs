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
    pub nbytes: u32,  // on-disk size
    pub version: u16,
    pub obj_len: u32, // uncompressed size
    pub datime: u32,
    pub key_len: u16,
    pub cycle: u16,
    pub seek_key: fileptr,
    pub seek_pdir: fileptr,
    pub class_name: String,
    pub name: String,
    pub title: String
}

impl RecordHeader {
    pub fn data_offset(&self) -> usize {
        let res = 27 + self.class_name.len() + self.name.len() + self.title.len() + 2;
            println!("data_offset: {}", res);
        return res;
    }

    pub fn data_ptr<'a>(&self, mmap: &'a [u8]) -> &'a [u8] {
        return &mmap[(self.seek_key as usize + self.data_offset())..];
    }

    pub fn is_compressed(&self) -> bool {
        return (self.obj_len as usize) + self.data_offset() != (self.nbytes as usize);
    }
}

impl MMapReader for RecordHeader {
    fn read(mmap: &[u8]) -> (Self, usize) {
        // assume file < 4GB -> offsets are 4 bytes
        let strs = &mmap[26..];
        let (class_name, off) = Self::read_string(strs);
        let (name, off2) = Self::read_string(&strs[off..]);
        let (title, off3) = Self::read_string(&strs[off+off2..]);
        let hdr = RecordHeader {
            nbytes     : read_be!(mmap, u32, 0, 4),
            version    : read_be!(mmap, u16, 4, 6),
            obj_len    : read_be!(mmap, u32, 6, 10),
            datime     : read_be!(mmap, u32, 10, 14),
            key_len    : read_be!(mmap, u16, 14, 16),
            cycle      : read_be!(mmap, u16, 16, 18),
            seek_key   : read_be!(mmap, u32, 18, 22),
            seek_pdir  : read_be!(mmap, u32, 22, 26),
            class_name,
            name,
            title,
        };
        return (hdr, 26 + off + off2 + off3)
    }
}

////////////////////////////////////////////////////////////////////////////////
//  Directory

#[allow(dead_code)]
#[derive(Debug)]
pub struct Directory {
     pub version: u16,
     pub c_time: u32,
     pub m_time: u32,
     pub n_bytes_keys: i32,
     pub n_bytes_name: i32,
     pub seek_dir: fileptr,
     pub seek_parent: fileptr,
     pub seek_keys: fileptr,
}

impl Directory {
   pub fn keys_offset(&self) -> usize {
       return self.seek_keys as usize;
   }

   pub fn keys_ptr<'a>(&self, mmap: &'a [u8]) -> &'a [u8] {
       return &mmap[self.keys_offset()..];
   }
}

impl MMapReader for Directory {
    fn read(src: &[u8]) -> (Self, usize) where Self: Sized {
        let dir = Directory {
            version: read_be!(src, u16, 0, 2),
            c_time: read_be!(src, u32, 2, 6),
            m_time: read_be!(src, u32, 6, 10),
            n_bytes_keys: read_be!(src, i32, 10, 14),
            n_bytes_name: read_be!(src, i32, 14, 18),
            seek_dir: read_be!(src, fileptr, 18, 22),
            seek_parent: read_be!(src, fileptr, 22, 26),
            seek_keys: read_be!(src, fileptr, 26, 30),
        };
        return (dir, 30)
    }
}


////////////////////////////////////////////////////////////////////////////////
//  KeyHeaderArray
#[allow(dead_code)]
#[derive(Debug)]
pub struct KeyHeaders {
    pub count: u32
    // pub key_offsets: Vec<fileptr>
}

impl MMapReader for KeyHeaders {
    fn read(src: &[u8]) -> (Self, usize) where Self: Sized {
        let count = read_be!(src, u32, 0, 4);
        let mut off = 4;
        for i in 0..count {
            let (key, _size) = RecordHeader::read(&src[off..]);
            println!("key {}: {:?} ({})", i, key, _size);
            off += key.nbytes as usize;
        }
        return (KeyHeaders { count }, off)
    }
}

////////////////////////////////////////////////////////////////////////////////
//  Compressed Object Header
// parsed out of code from inofficial go root implementation
// https://github.com/go-hep/hep/blob/main/groot/rbytes/rbuffer.go#L99 and following

#[allow(dead_code)]
#[derive(Debug)]
pub struct CompressedObjHeader {
    pub magic_bytes: [u8; 2],
    pub header: [u8; 7],
}

impl MMapReader for CompressedObjHeader {
    fn read(mmap: &[u8]) -> (Self, usize) {
        let hdr = CompressedObjHeader {
            magic_bytes: [mmap[0], mmap[1]],
            header: [mmap[2], mmap[3], mmap[4], mmap[5], mmap[6], mmap[7], mmap[8]],
        };
        // print bytes as ascii
        let magic_str = String::from_utf8_lossy(&hdr.magic_bytes);
        let header_str = String::from_utf8_lossy(&hdr.header);
        println!("magic: {}, header: {}", magic_str, header_str);
        return (hdr, 9)
    }
}



#[allow(dead_code)]
#[derive(Debug)]
pub struct ObjName {
    pub version: u16,
    pub version2: u16,
    pub id: u32,
    pub bits: u32,
    pub name: String,
    pub title: String
}

impl MMapReader for ObjName {
    fn read(src: &[u8]) -> (Self, usize) {
        let ver = read_be!(src, u16, 0, 2);
        println!("ver: {}", ver);
        let ver2 = read_be!(src, u16, 2, 4);
        println!("ver2: {}", ver2);
        let id = read_be!(src, u32, 4, 8);
        println!("id: {}", id);
        let bits = read_be!(src, u32, 8, 12);
        println!("bits: {}", bits);
        let (name, off) = Self::read_string(&src[12..]);
        println!("name: {}", name);
        let (title, off2) = Self::read_string(&src[12+off..]);
        println!("title: {}", title);
        return (ObjName { version: ver, version2: ver2, id, bits, name, title }, 12 + off + off2)
    }
}


#[allow(dead_code)]
#[derive(Debug)]
pub struct ObjHeader {
    pub version: u16,
    pub name: ObjName
}

impl MMapReader for ObjHeader {
    fn read(src: &[u8]) -> (Self, usize) {
        let ver = read_be!(src, u16, 0, 2);
        let (name, off) = ObjName::read(&src[2..]);
        return (ObjHeader { version: ver, name }, off + 2)
    }
}
