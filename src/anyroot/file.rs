use crate::anyroot::reader::*;

// ROOT file format
// from https://github.com/root-project/root/blob/master/io/io/src/TFile.cxx
// and  https://pkg.go.dev/go-hep.org/x/hep/groot?utm_source=godoc#hdr-File_layout
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
#[allow(dead_code)]
#[derive(Debug)]
pub struct RootHeader {
    pub version: u32,
    pub begin: u32,
    pub end: u32,
    pub seek_free: fileptr,
    pub nbytes_free: u32,
    pub nfree: u32,
    pub nbytes_name: u32,
    pub units: u8,
    pub compress: u32,
    pub seek_info: fileptr,
    pub nbytes_info: u32,
    pub uuid: u128
}

impl MMapReader for RootHeader {
    // interface
    fn read(mmap: &[u8]) -> (Self, usize) {
        // assume file < 4GB -> offsets are 4 bytes
        let res = RootHeader {
            version     : read_be!(mmap, u32, 4, 8),
            begin       : read_be!(mmap, u32, 8, 12),
            end         : read_be!(mmap, u32, 12, 16),
            seek_free   : read_be!(mmap, fileptr, 16, 20),
            nbytes_free : read_be!(mmap, u32, 20, 24),
            nfree       : read_be!(mmap, u32, 24, 28),
            nbytes_name : read_be!(mmap, u32, 28, 32),
            units       : read_be!(mmap, u8, 32, 33),
            compress    : read_be!(mmap, u32, 33, 37),
            seek_info   : read_be!(mmap, fileptr, 37, 41),
            nbytes_info : read_be!(mmap, u32, 41, 45),
            uuid        : read_be!(mmap, u128, 45, 61)
        };
        return (res, 61);
    }
}
