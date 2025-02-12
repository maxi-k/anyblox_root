use memmap::Mmap;
mod anyroot;
use crate::anyroot::*;

//#[derive(Debug)]
// struct Directory {
//     version: u16,
//     c_time: u32,
//     m_time: u32,
//     n_bytes_keys: i32,
//     n_bytes_name: i32,
//     seek_dir: fileptr,
//     seek_parent: fileptr,
//     seek_keys: fileptr,
// }

// fn read_directory(mmap: &[u8]) -> Directory {
//     return Directory {
//         version : read_be!(mmap, u16, 0, 2)
//     }
// } 

// #[derive(Debug)]
// pub(crate) struct TStreamerElement {
//     ver: u16,
//     name: TNamed,
//     el_type: TypeID,
//     size: i32,
//     array_len: i32,
//     array_dim: i32,
//     max_idx: Vec<u32>,
//     type_name: String,
//     // For ver == 3
//     // pub(crate) xmin: f32,
//     // pub(crate) xmax: f32,
//     // pub(crate) factor: f32,
// }
//
// #[derive(Debug)]
// struct TStreamerInfo {
//     tstreamerinfo_ver: u16,
//     named: TNamed,
//     checksum: u32,
//     new_class_version: u32,
//     data_members: Vec<TStreamer>,
// }

fn main() {
    //let filename = "../b2hhh.zstd.root";
    let filename = "../cern.ch:84000.root";
    println!("Opening file: {}", filename);
    let path = std::path::Path::new(filename);
    // mmap the file
    let file = std::fs::File::open(path).unwrap();
    let mmap = unsafe { Mmap::map(&file).unwrap() };
    // read the header according to the spec
    let (header, _size) = RootHeader::read(&mmap);
    println!("file header: {:?}", header);
    // read directory object header according to the spec
    let (obj_header, _size) = RecordHeader::read(&mmap[header.seek_info as usize..]);
    println!("streamer info header: {:?}", obj_header);
    // read first object
    let (obj_header, _size) = RecordHeader::read(&mmap[header.begin as usize..]);
    println!("first object: {:?}", obj_header);
}
