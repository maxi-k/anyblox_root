use memmap::Mmap;
mod anyroot;
use crate::anyroot::*;


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
    println!("file header: {:?} ({})", header, _size);

    // read seek info object header according to the spec
    let (obj_header, _size) = RecordHeader::read(&mmap[header.seek_info as usize..]);
    println!("streamer info header: {:?} ({})", obj_header, _size);

    // read first object
    let (obj_header, _size) = RecordHeader::read(&mmap[header.begin as usize..]);
    println!("first object: {:?} ({})", obj_header, _size);
    assert!(!obj_header.is_compressed());

    // read directory
    let (dir, _size) = Directory::read(header.dir_ptr(&mmap));
    println!("directory: {:?} ({})", obj_header, _size);

    // read key list record
    let (obj_header, _size) = RecordHeader::read(dir.keys_ptr(&mmap));
    println!("key list record: {:?} ({})", obj_header, _size);

    let (keys, _size) = KeyHeaders::read(&obj_header.data_ptr(&mmap));
    println!("keys: {:?} ({})", keys, _size);

    let (fkey, _size) = RecordHeader::read(keys.first_key_ptr(&obj_header.data_ptr(&mmap)));
    println!("first key: {:?} ({})", fkey, _size);

    let (tree, _size) = TreeObj::read(fkey.data_ptr(&mmap));
    println!("tree: {:?} ({})", tree, _size);

    // // read first object content
    // let start = obj_header.data_start(&mmap[0..]);
    // // sanity check: print mmap start ptr and this ptr, diff
    // println!("mmap start: {:p}, this start: {:p}, diff: {}", &mmap[0], start, (unsafe { start.as_ptr().byte_offset_from(mmap.as_ptr()) }) as usize);
    // let (obj_header2, _size) = ObjHeader::read(start);
    // println!("first object data: {:?} ({})", obj_header2, _size);
}
