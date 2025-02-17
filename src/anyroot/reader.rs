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

  fn take<'a>(src: &'a [u8]) -> (Self, &'a [u8]) where Self: Sized {
    let (val, len) = Self::read(src);
    return (val, &src[len..])
  }

  fn read_string(src: &[u8]) -> (String, usize) {
    let len = read_be!(src, u8, 0, 1);
    let (start, len) = match (src, len)  {
      (src, 255) => (&src[4..], read_be!(src, u32, 1, 4) as usize),
      (src, val) => (&src[1..], val as usize)
    };
    println!("read_string: len={}", len);
    return (String::from_utf8(start[..len].to_vec()).unwrap(), len + 1);
  }
}
