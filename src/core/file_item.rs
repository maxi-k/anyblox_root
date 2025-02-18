use failure::Error;
use nom::multi::length_value;

use crate::core::{checked_byte_count, decompress, Context, Source, TKeyHeader};
use crate::tree_reader::{ttree, Tree};

/// Describes a single item within this file (e.g. a `Tree`)
#[derive(Debug)]
pub struct FileItem {
    source: Source,
    tkey_hdr: TKeyHeader,
}

impl FileItem {
    /// New file item from the information in a TKeyHeader and the associated file
    pub(crate) fn new(tkey_hdr: &TKeyHeader, source: Source) -> FileItem {
        FileItem {
            source,
            tkey_hdr: tkey_hdr.to_owned(),
        }
    }

    /// Information about this file item in Human readable form
    pub fn verbose_info(&self) -> String {
        format!("{:#?}", self.tkey_hdr)
    }
    pub fn name(&self) -> String {
        format!(
            "`{}` of type `{}`",
            self.tkey_hdr.obj_name, self.tkey_hdr.class_name
        )
    }

    fn get_buffer(&self) -> Result<Vec<u8>, Error> {
        let start = self.tkey_hdr.seek_key + self.tkey_hdr.key_len as u64;
        let len = self.tkey_hdr.total_size - self.tkey_hdr.key_len as u32;
        let comp_buf = self.source.fetch(start, len as u64)?;

        let buf = if self.tkey_hdr.total_size < self.tkey_hdr.uncomp_len {
            // Decompress the read buffer; buf is Vec<u8>
            println!("decompressing fileitem buffer of length {}MB", len/ 1024/1024);
            let (_, buf) = decompress(comp_buf.as_slice()).unwrap();
            buf
        } else {
            comp_buf
        };
        println!("inmem fileitem buffer of length {}MB", buf.len()/ 1024/1024);
        Ok(buf)
    }

    pub(crate) fn get_context<'s>(&self) -> Result<Context, Error> {
        let buffer = self.get_buffer()?;
        let k_map_offset = 2;
        Ok(Context {
            source: self.source.clone(),
            offset: (self.tkey_hdr.key_len + k_map_offset) as u64,
            s: buffer,
        })
    }

    /// Parse this `FileItem` as a `Tree`
    pub fn as_tree(&self) -> Result<Tree, Error> {
        let ctx = self.get_context()?;
        let buf = ctx.s.as_slice();

        let res = length_value(checked_byte_count, |i| ttree(i, &ctx))(buf);
        match res {
            Ok((_, obj)) => Ok(obj),
            Err(nom::Err::Error(e)) | Err(nom::Err::Failure(e)) => {
                Err(format_err!("Supplied parser failed! {:?}", e))
            }
            _ => panic!(),
        }
    }
}
