// helper macro for reading type T from byte slice in big endian
macro_rules! __read_be {
    ($buf: expr, $type:ident, $start:expr, $end:expr) => {
        $type::from_be_bytes($buf[$start..$end].try_into().unwrap())
    };
}

pub(crate) use __read_be as read_be;

// pointer inside a file. we only support files < 2GB for simplicity.
// The full root spec is dynamic, e.g., the file format uses 32bit fields
// for small files and 64bit fields for larger files, which is annoying AF.
#[allow(non_camel_case_types)]
pub type fileptr = u32;

pub trait MMapReader {
  fn read(src: &[u8]) -> (Self, usize) where Self: Sized;
}
