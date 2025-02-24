
// - best way is probably creating a record batch from an iterator, see
//      - https://docs.rs/arrow/latest/arrow/record_batch/struct.RecordBatch.html#method.try_from_iter
//      - but would need to implement iterator over our parser
// - could consume entire row group, cache as record batch, then return slices according to requested row range
//      - https://docs.rs/arrow/latest/arrow/array/struct.RecordBatch.html#method.slice

use std::{sync::Arc};

use crate::{anyblox::{ColumnProjection, rowgroup::RowGroup}, tree_reader::Tree};
use arrow::{
    array::*,
    datatypes::*,
    record_batch::RecordBatch,
};
use nom::number::complete::*;

pub fn string_to_arrow_type(s: &str) -> DataType {
    // TODO more types
    match s {
        "f64" => DataType::Float64,
        "u64" => DataType::UInt64,
        "i64" => DataType::Int64,
        "f32" => DataType::Float32,
        "u32" => DataType::UInt32,
        "i32" => DataType::Int32,
        "bool" => DataType::Boolean,
       _ => panic!("unknown data type {}", s)
    }
}

pub fn branches_to_arrow_schema(branches: &[(String, String)], cols: u64) -> Schema {
    let mask = ColumnProjection::from_u64(cols);
    let fields = branches
        .iter()
        .enumerate()
        .filter(|(idx, _b)| mask.contains(*idx as u32))
        .map(|(_idx, b)| Field::new(b.0.clone(), string_to_arrow_type(b.1.as_str()), false))
        .collect::<Vec<Field>>(); // TODO ^ nullability always false, they have *_valid columns though
    Schema::new(fields)
}

pub fn tree_to_arrow_schema(tree: &Tree, cols: u64) -> Schema {
    branches_to_arrow_schema(&tree.main_branch_names_and_types().as_slice(), cols)
}

fn be_bool<I, E>(input: I) -> nom::IResult<I, bool, E>
where
    I: nom::InputIter<Item = u8> + nom::InputTake + nom::InputLength + nom::Slice<std::ops::RangeFrom<usize>>,
    E: nom::error::ParseError<I>,
{
    nom::combinator::map(le_u8::<I, E>, |b| b != 0)(input)
}

pub fn rowgroup_to_record_batch(mmap: &[u8], colmask: u64, rg: &RowGroup, sc: Arc<Schema>) -> RecordBatch {
    let mut arrays: Vec<ArrayRef> = Vec::new();
    arrays.reserve(colmask.count_ones() as usize);
    let arrays = rg.decode(mmap, colmask, arrays, |mut cols, cursor, data| {
        let coltype = sc.field(cursor.projected_col_idx).data_type();
        let cnt = rg.count as usize;
        macro_rules! parse_array_nom(
            ($arr:ident, $parser:ident, $cnt:expr) => {
                nom::multi::fold_many_m_n(
                    $cnt, $cnt, $parser::<&[u8], nom::error::Error<&[u8]>>,
                    move || $arr::builder($cnt), |mut bld, val| {
                        bld.append_value(val);
                        bld
                    })(data).unwrap().1.finish()
            }
        );
        macro_rules! unsafe_cast_array(
            ($arr:ident, $type:ident, $cnt:expr) => {
                // append_slice (transmute(data))
                (|| { let bytes = bytes::Bytes::copy_from_slice(data);
                      let buf = $arr::new(arrow::buffer::ScalarBuffer::from(arrow::buffer::Buffer::from(bytes)), None);
                      buf
                })()
            }
        );
        // slower variant returning native-endian arrays
        #[cfg(feature = "slow_endian_parsing")]
        macro_rules! parse_array(
            ($type:ident, $arr:ident, $parser:ident, $cnt:expr) => {parse_array_nom!($arr, $parser, $cnt)}
        );
        // unsafe variant returning big-endian arrays (root format)
        #[cfg(not(feature = "slow_endian_parsing"))]
        macro_rules! parse_array(
            ($type:ident, $arr:ident, $parser:ident, $cnt:expr) => {unsafe_cast_array!($arr, $type, $cnt)}
        );
        let arr: ArrayRef = match coltype {
            DataType::UInt32 => {
                assert!(cursor.byte_count/4 == cnt);
                Arc::new(parse_array!(UInt32Type, UInt32Array, be_u32, cnt))
            }
            DataType::Int32 => {
                assert!(cursor.byte_count/4 == cnt);
                Arc::new(parse_array!(Int32Type, Int32Array, le_i32, cnt))
            }
            DataType::Float32 => {
                assert!(cursor.byte_count/4 == cnt);
                Arc::new(parse_array!(Float32Type, Float32Array, be_f32, cnt))
            }
            DataType::UInt64 => {
                assert!(cursor.byte_count/8 == cnt);
                Arc::new(parse_array!(UInt64Type, UInt64Array, be_u64, cnt))
            }
            DataType::Int64 => {
                assert!(cursor.byte_count/8 == cnt);
                Arc::new(parse_array!(Int64Type, Int64Array, be_i64, cnt))
            }
            DataType::Float64 => {
                assert!(cursor.byte_count/8 == cnt);
                Arc::new(parse_array!(Float64Type, Float64Array, be_f64, cnt))
            }
            DataType::Boolean => {
                assert!(cursor.byte_count == cnt);
                // XXX arrow primitive type boolean
                Arc::new(parse_array_nom!(BooleanArray, be_bool, cnt))
            }
            _ => panic!("unsupported data type in rowgroup_to_record_batch"),
        };
        assert!(cursor.projected_col_idx == cols.len());
        cols.push(arr);
        cols
    });
    // XXX reuse schema by passing in arc into this fn?
    RecordBatch::try_new(sc, arrays).unwrap()
}
