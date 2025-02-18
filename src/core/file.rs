use std::fmt;

use failure::Error;
use nom::{
    self,
    bytes::complete::tag,
    combinator::map,
    number::complete::{be_i16, be_i32, be_u128, be_u16, be_u32, be_u64, be_u8},
    IResult,
};

use uuid::Uuid;

use crate::{
    code_gen::rust::{ToNamedRustParser, ToRustStruct},
    core::tstreamer::streamers,
    core::*,
    MAP_OFFSET,
};

/// Size of serialized `FileHeader` in bytes
const FILE_HEADER_SIZE: u64 = 75;

/// Size of serialized TDirectory. Depending on the ROOT version this
/// may use 32 or 64 bit pointers. This is the maximal (64 bit size).
const TDIRECTORY_MAX_SIZE: u64 = 42;

/// `RootFile` wraps the most basic information of a ROOT file.
#[derive(Debug)]
pub struct RootFile {
    pub(crate) source: Source,
    pub(crate) hdr: FileHeader,
    pub(crate) items: Vec<FileItem>,
}

#[derive(Debug, PartialEq)]
struct FileHeader {
    version: i32,
    begin: i32,
    end: u64,
    seek_free: u64,
    nbytes_free: i32,
    n_entries_free: i32,
    n_bytes_name: i32,
    pointer_size: u8,
    compression: i32,
    seek_info: SeekPointer,
    nbytes_info: i32,
    uuid: Uuid,
    seek_dir: SeekPointer,
}

#[derive(Debug, PartialEq)]
pub struct Directory {
    version: i16,
    c_time: u32,
    m_time: u32,
    n_bytes_keys: i32,
    n_bytes_name: i32,
    seek_dir: SeekPointer,
    seek_parent: SeekPointer,
    seek_keys: SeekPointer,
}

/// Parse opening part of a root file
fn file_header(i: &[u8]) -> IResult<&[u8], FileHeader> {
    fn version_dep_int(i: &[u8], is_64_bit: bool) -> IResult<&[u8], u64> {
        if is_64_bit {
            be_u64(i)
        } else {
            let (i, end) = be_u32(i)?;
            Ok((i, end as u64))
        }
    }
    let (i, _) = tag("root")(i)?;
    let (i, version) = be_i32(i)?;
    let is_64_bit = version > 1000000;
    let (i, begin) = be_i32(i)?;
    let (i, end) = version_dep_int(i, is_64_bit)?;
    let (i, seek_free) = version_dep_int(i, is_64_bit)?;
    let (i, nbytes_free) = be_i32(i)?;
    let (i, n_entries_free) = be_i32(i)?;
    let (i, n_bytes_name) = be_i32(i)?;
    let (i, pointer_size) = be_u8(i)?;
    let (i, compression) = be_i32(i)?;
    let (i, seek_info) = version_dep_int(i, is_64_bit)?;
    let (i, nbytes_info) = be_i32(i)?;
    let (i, _uuid_version) = be_u16(i)?;
    let (i, uuid) = be_u128(i)?;

    let uuid = Uuid::from_u128(uuid);
    let seek_dir = (begin + n_bytes_name) as u64;
    Ok((
        i,
        FileHeader {
            version,
            begin,
            end,
            seek_free,
            nbytes_free,
            n_entries_free,
            n_bytes_name,
            pointer_size,
            compression,
            seek_info,
            nbytes_info,
            uuid,
            seek_dir,
        },
    ))
}

/// Parse a file-pointer based on the version of the file
fn versioned_pointer(input: &[u8], version: i16) -> nom::IResult<&[u8], u64> {
    if version > 1000 {
        be_u64(input)
    } else {
        map(be_i32, |val| val as u64)(input)
    }
}

/// Directory within a root file; exists on ever file
fn directory(input: &[u8]) -> nom::IResult<&[u8], Directory> {
    let (input, version) = be_i16(input)?;
    let (input, c_time) = be_u32(input)?;
    let (input, m_time) = be_u32(input)?;
    let (input, n_bytes_keys) = be_i32(input)?;
    let (input, n_bytes_name) = be_i32(input)?;
    let (input, seek_dir) = versioned_pointer(input, version)?;
    let (input, seek_parent) = versioned_pointer(input, version)?;
    let (input, seek_keys) = versioned_pointer(input, version)?;
    Ok((
        input,
        Directory {
            version,
            c_time,
            m_time,
            n_bytes_keys,
            n_bytes_name,
            seek_dir,
            seek_parent,
            seek_keys,
        },
    ))
}

impl RootFile {
    /// Open a new ROOT file either from a `Url`
    /// (not available on `wasm32`).
    pub fn new<S: Into<Source>>(source: S) -> Result<Self, Error> {
        let source = source.into();
        let hdr = source.fetch(0, FILE_HEADER_SIZE).and_then(|buf| {
            file_header(&buf)
                .map_err(|_| format_err!("Failed to parse file header"))
                .map(|(_i, o)| o)})?;
        // Jump to the TDirectory and parse it
        let dir = source
            .fetch(hdr.seek_dir, TDIRECTORY_MAX_SIZE)
            .and_then(|buf| {
                directory(&buf)
                    .map_err(|_| format_err!("Failed to parse TDirectory"))
                    .map(|(_i, o)| o)
            })?;
        let tkey_of_keys = source
            .fetch(dir.seek_keys, dir.n_bytes_keys as u64)
            .and_then(|buf| {
                tkey(&buf)
                    .map_err(|_| format_err!("Failed to parse TKeys"))
                    .map(|(_i, o)| o)
            })?;
        let keys = match tkey_headers(&tkey_of_keys.obj) {
            Ok((_, hdrs)) => Ok(hdrs),
            _ => Err(format_err!("Expected TKeyHeaders")),
        }?;
        let items = keys
            .iter()
            .map(|k_hdr| FileItem::new(k_hdr, source.clone()))
            .collect();

        Ok(RootFile { source, hdr, items })
    }

    pub fn get_streamer_context(&self) -> Result<Context, Error> {
        let seek_info_len = (self.hdr.nbytes_info + 4) as u64;
        let info_key = self
            .source
            .fetch(self.hdr.seek_info, seek_info_len)
            .map(|buf| tkey(&buf).unwrap().1)?;

        let key_len = info_key.hdr.key_len;
        Ok(Context {
            source: self.source.clone(),
            offset: key_len as u64 + MAP_OFFSET,
            s: info_key.obj,
        })
    }

    /// Slice of the items contained in this file
    pub fn items(&self) -> &[FileItem] {
        &self.items
    }

    /// Translate the streamer info of this file to a YAML file
    pub fn streamer_infos(&self) -> Result<Vec<TStreamerInfo>, Error> {
        let ctx = self.get_streamer_context()?;
        let buf = ctx.s.as_slice();
        let (_, streamer_vec) =
            streamers(buf, &ctx).map_err(|_| format_err!("Failed to parse TStreamers"))?;
        Ok(streamer_vec)
    }

    /// Translate the streamer info of this file to a YAML file
    pub fn streamer_info_as_yaml<W: fmt::Write>(&self, s: &mut W) -> Result<(), Error> {
        for el in &self.streamer_infos()? {
            writeln!(s, "{:#}", el.to_yaml())?;
        }
        Ok(())
    }

    /// Generate Rust code from the streamer info of this file
    pub fn streamer_info_as_rust<W: fmt::Write>(&self, s: &mut W) -> Result<(), Error> {
        // Add necessary imports at the top of the file
        writeln!(
            s,
            "{}",
            quote! {
                use std::marker::PhantomData;
                use nom::*;
                use parsers::*;
                use parsers::utils::*;
                use core_types::*;
            }
        )?;
        let streamer_infos = self.streamer_infos()?;
        // generate structs
        for el in &streamer_infos {
            // The structs contain comments which introduce line breaks; i.e. readable
            writeln!(s, "{}", el.to_struct())?;
        }

        // generate parsers
        for el in &streamer_infos {
            // The parsers have no comments, but are ugly; We introduce some
            // Linebreaks here to not have rustfmt choke later (doing it later
            // is inconvinient since the comments in the structs might contain
            // the patterns
            let parsers = el.to_named_parser().to_string();
            let parsers = parsers.replace(',', ",\n");
            let parsers = parsers.replace(">>", ">>\n");
            // macro names are generated as my_macro ! (...) by `quote`
            let parsers = parsers.replace(" ! (", "!(");
            writeln!(s, "{}", parsers)?;
        }
        Ok(())
    }
}
