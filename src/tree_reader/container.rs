use failure::Error;
use nom::combinator::rest;
use nom::number::complete::*;
use nom::*;

use crate::core::*;

#[derive(Debug, Clone)]
pub enum Container {
    /// Decompressed content of a `TBasket`
    InMemory(Vec<u8>),
    /// Filename, start byte, and len of a `TBasket` on disk
    OnDisk(Source, u64, u64),
}

impl Container {
    /// Return the number of entries and the data; reading it from disk if necessary
    pub fn raw_data(self) -> Result<(u32, Vec<u8>), Error> {
        let buf = match self {
            Container::InMemory(buf) => buf,
            Container::OnDisk(source, seek, len) => source.fetch(seek, len)?,
        };
        match tbasket2vec(buf.as_slice()) {
            Ok((_, v)) => Ok(v),
            _ => Err(format_err!("tbasket2vec parser failed")),
        }
    }
    // /// For debugging: Try to find the file of this container. Out of luck if the container was inlined
    // pub(crate) fn file(&self) -> Option<PathBuf> {
    //     match *self {
    //         // No file name available
    //         Container::InMemory(_) => None,
    //         Container::OnDisk(ref p, _, _) => Some(p.to_owned())
    //     }
    // }
}

pub struct BasketHeader<'a> {
    pub header: TKeyHeader,
    pub version: u16,
    pub buf_size: u32,
    pub entry_size: u32,
    pub n_entry_buf: u32,
    pub last: u32,
    pub flag: i8,
    pub buf: &'a [u8],
}

impl BasketHeader<'_> {
    pub fn useful_bytes(&self) -> usize {
        // Not the whole buffer is filled, no, no, no, that
        // would be to easy! Its only filled up to `last`,
        // whereby we have to take the key_len into account...
        (self.last - self.header.key_len as u32) as usize

    }

    pub fn is_compressed(&self) -> bool {
        self.header.uncomp_len as usize > self.buf.len()
    }

    pub fn decode_into(&self, output: &mut [u8]) -> usize {
        if self.is_compressed() {
            let max_size = self.header.uncomp_len as usize;
            let mut outbuf = &mut output[..max_size];
            let nbyte = decompress_into(&self.buf, &mut outbuf).unwrap().1;
            assert!(nbyte <= outbuf.len());
            assert!(nbyte <= self.useful_bytes());
        } else {
            output.copy_from_slice(&self.buf[..self.useful_bytes()]);
        }
        self.useful_bytes()
    }
}

pub fn basket_header(input: &[u8]) -> IResult<&[u8], BasketHeader> {
    let (input, header) = tkey_header(input)?;
    let (input, version) = be_u16(input)?;
    let (input, buf_size) = be_u32(input)?;
    let (input, entry_size) = be_u32(input)?;
    let (input, n_entry_buf) = be_u32(input)?;
    let (input, last) = be_u32(input)?;
    let (input, flag) = be_i8(input)?;
    let (input, buf) = rest(input)?;
    Ok((input, BasketHeader {
        header,
        version,
        buf_size,
        entry_size,
        n_entry_buf,
        last,
        flag,
        buf
    }))
}


/// Return a tuple indicating the number of elements in this basket
/// and the content as a Vec<u8>
fn tbasket2vec(input: &[u8]) -> IResult<&[u8], (u32, Vec<u8>)> {
    let (input, hdr) = basket_header(input)?;
    let buf = if hdr.header.uncomp_len as usize > hdr.buf.len() {
        // println!("decompressing container!");
        decompress(hdr.buf).unwrap().1
    } else {
        hdr.buf.to_vec()
    };
    Ok((
        input,
        (hdr.n_entry_buf, buf.as_slice()[..hdr.useful_bytes()].to_vec()),
    ))
}

#[cfg(test)]
mod tests {
    use crate::core::tkey_header;
    use nom::*;
    use std::fs::File;
    use std::io::{BufReader, Read, Seek, SeekFrom};

    use super::tbasket2vec;

    #[test]
    fn basket_simple() {
        let path = "./src/test_data/simple.root";
        let f = File::open(path).unwrap();
        let mut reader = BufReader::new(f);
        // Go to first basket
        reader.seek(SeekFrom::Start(218)).unwrap();
        // size from fbasketbytes
        let mut buf = vec![0; 86];
        // let mut buf = vec![0; 386];
        reader.read_exact(&mut buf).unwrap();

        println!("{}", buf.to_hex(16));
        println!("{:?}", tkey_header(&buf));
        // println!("{:#?}", tbasket(&buf, be_u32));
        println!("{:#?}", tbasket2vec(&buf));
    }

    // /// Test the first basket of the "Tracks.fP[5]" branch
    // #[test]
    // fn basket_esd() {
    //     // This test is broken since the numbers were hardcoded for a specific file
    //     use alice_open_data;
    //     let path = alice_open_data::test_file().unwrap();

    //     let f = File::open(&path).unwrap();
    //     let mut reader = BufReader::new(f);
    //     // Go to first basket
    //     reader.seek(SeekFrom::Start(77881)).unwrap();
    //     // size from fbasketbytes
    //     let mut buf = vec![0; 87125];
    //     reader.read_exact(&mut buf).unwrap();

    //     println!("{:?}", tkey_header(&buf).unwrap().1);
    //     // println!("{:#?}", tbasket(&buf, |i| count!(i, be_f32, 15)).unwrap().1);
    //     println!("{:#?}", tbasket2vec(&buf));
    // }
}
