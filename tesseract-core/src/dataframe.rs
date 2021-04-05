use anyhow::{Error, format_err};


#[derive(Debug)]
pub struct DataFrame {
    pub columns: Vec<Column>,
}

impl DataFrame {
    pub fn new() -> Self {
        DataFrame {
            columns: vec![],
        }
    }

    pub fn from_vec(columns: Vec<Column>) -> Self {
        DataFrame {
            columns
        }
    }

    pub fn len(&self) -> usize {
        if let Some(col) = self.columns.get(0) {
            match col.column_data {
                ColumnData::Int8(ref ns) => ns.len(),
                ColumnData::Int16(ref ns) => ns.len(),
                ColumnData::Int32(ref ns) => ns.len(),
                ColumnData::Int64(ref ns) => ns.len(),
                ColumnData::UInt8(ref ns) => ns.len(),
                ColumnData::UInt16(ref ns) => ns.len(),
                ColumnData::UInt32(ref ns) => ns.len(),
                ColumnData::UInt64(ref ns) => ns.len(),
                ColumnData::Float32(ref ns) => ns.len(),
                ColumnData::Float64(ref ns) => ns.len(),
                ColumnData::Text(ref ss) => ss.len(),
                ColumnData::NullableInt8(ref ns) => ns.len(),
                ColumnData::NullableInt16(ref ns) => ns.len(),
                ColumnData::NullableInt32(ref ns) => ns.len(),
                ColumnData::NullableInt64(ref ns) => ns.len(),
                ColumnData::NullableUInt8(ref ns) => ns.len(),
                ColumnData::NullableUInt16(ref ns) => ns.len(),
                ColumnData::NullableUInt32(ref ns) => ns.len(),
                ColumnData::NullableUInt64(ref ns) => ns.len(),
                ColumnData::NullableFloat32(ref ns) => ns.len(),
                ColumnData::NullableFloat64(ref ns) => ns.len(),
                ColumnData::NullableText(ref ss) => ss.len(),
            }
        } else {
            0
        }
    }
}

#[derive(Debug)]
pub struct Column {
    pub name: String,
    pub column_data: ColumnData,
}

impl Column {
    pub fn new(name: String, column_data: ColumnData) -> Self {
        Column {
            name,
            column_data,
        }
    }

    pub fn column_data(&mut self) ->&mut ColumnData {
        &mut self.column_data
    }

    /// Sort column entries for all types, but floats.
    pub fn sort_column_data(&mut self) -> Result<(), Error> {
        match self.column_data {
            ColumnData::Int8(ref mut v) => v.sort(),
            ColumnData::Int16(ref mut v) => v.sort(),
            ColumnData::Int32(ref mut v) => v.sort(),
            ColumnData::Int64(ref mut v) => v.sort(),
            ColumnData::UInt8(ref mut v) => v.sort(),
            ColumnData::UInt16(ref mut v) => v.sort(),
            ColumnData::UInt32(ref mut v) => v.sort(),
            ColumnData::UInt64(ref mut v) => v.sort(),
            ColumnData::Float32(_) => {
                return Err(format_err!("Cannot sort Float32 column"));
            },
            ColumnData::Float64(_) => {
                return Err(format_err!("Cannot sort Float64 column"));
            },
            ColumnData::Text(ref mut v) => v.sort(),
            ColumnData::NullableInt8(ref mut v) => v.sort(),
            ColumnData::NullableInt16(ref mut v) => v.sort(),
            ColumnData::NullableInt32(ref mut v) => v.sort(),
            ColumnData::NullableInt64(ref mut v) => v.sort(),
            ColumnData::NullableUInt8(ref mut v) => v.sort(),
            ColumnData::NullableUInt16(ref mut v) => v.sort(),
            ColumnData::NullableUInt32(ref mut v) => v.sort(),
            ColumnData::NullableUInt64(ref mut v) => v.sort(),
            ColumnData::NullableFloat32(_) => {
                return Err(format_err!("Cannot sort NullableFloat32 column"));
            },
            ColumnData::NullableFloat64(_) => {
                return Err(format_err!("Cannot sort NullableFloat64 column"));
            },
            ColumnData::NullableText(ref mut v) => v.sort(),
        }

        Ok(())
    }

    /// DataFrame columns can come in many different types. This function converts
    /// all data to a common type (String).
    pub fn stringify_column_data(&self) -> Vec<String> {
        return match &self.column_data {
            ColumnData::Int8(v) => v.iter().map(|&e| e.to_string()).collect(),
            ColumnData::Int16(v) => v.iter().map(|&e| e.to_string()).collect(),
            ColumnData::Int32(v) => v.iter().map(|&e| e.to_string()).collect(),
            ColumnData::Int64(v) => v.iter().map(|&e| e.to_string()).collect(),
            ColumnData::UInt8(v) => v.iter().map(|&e| e.to_string()).collect(),
            ColumnData::UInt16(v) => v.iter().map(|&e| e.to_string()).collect(),
            ColumnData::UInt32(v) => v.iter().map(|&e| e.to_string()).collect(),
            ColumnData::UInt64(v) => v.iter().map(|&e| e.to_string()).collect(),
            ColumnData::Float32(v) => v.iter().map(|&e| e.to_string()).collect(),
            ColumnData::Float64(v) => v.iter().map(|&e| e.to_string()).collect(),
            ColumnData::Text(v) => v.to_vec(),
            ColumnData::NullableInt8(v) => {
                v.iter().map(|&e| {
                    match e {
                        Some(e) => e.to_string(),
                        None => "".to_string()
                    }
                }).collect()
            },
            ColumnData::NullableInt16(v) => {
                v.iter().map(|&e| {
                    match e {
                        Some(e) => e.to_string(),
                        None => "".to_string()
                    }
                }).collect()
            },
            ColumnData::NullableInt32(v) => {
                v.iter().map(|&e| {
                    match e {
                        Some(e) => e.to_string(),
                        None => "".to_string()
                    }
                }).collect()
            },
            ColumnData::NullableInt64(v) => {
                v.iter().map(|&e| {
                    match e {
                        Some(e) => e.to_string(),
                        None => "".to_string()
                    }
                }).collect()
            },
            ColumnData::NullableUInt8(v) => {
                v.iter().map(|&e| {
                    match e {
                        Some(e) => e.to_string(),
                        None => "".to_string()
                    }
                }).collect()
            },
            ColumnData::NullableUInt16(v) => {
                v.iter().map(|&e| {
                    match e {
                        Some(e) => e.to_string(),
                        None => "".to_string()
                    }
                }).collect()
            },
            ColumnData::NullableUInt32(v) => {
                v.iter().map(|&e| {
                    match e {
                        Some(e) => e.to_string(),
                        None => "".to_string()
                    }
                }).collect()
            },
            ColumnData::NullableUInt64(v) => {
                v.iter().map(|&e| {
                    match e {
                        Some(e) => e.to_string(),
                        None => "".to_string()
                    }
                }).collect()
            },
            ColumnData::NullableFloat32(v) => {
                v.iter().map(|&e| {
                    match e {
                        Some(e) => e.to_string(),
                        None => "".to_string()
                    }
                }).collect()
            },
            ColumnData::NullableFloat64(v) => {
                v.iter().map(|&e| {
                    match e {
                        Some(e) => e.to_string(),
                        None => "".to_string()
                    }
                }).collect()
            },
            ColumnData::NullableText(v) => {
                v.iter().map(|e| {
                    match e {
                        Some(e) => e.clone(),
                        None => "".to_string()
                    }
                }).collect()
            },
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum ColumnData {
    Int8(Vec<i8>),
    Int16(Vec<i16>),
    Int32(Vec<i32>),
    Int64(Vec<i64>),
    UInt8(Vec<u8>),
    UInt16(Vec<u16>),
    UInt32(Vec<u32>),
    UInt64(Vec<u64>),
    Float32(Vec<f32>),
    Float64(Vec<f64>),
    Text(Vec<String>),
    NullableInt8(Vec<Option<i8>>),
    NullableInt16(Vec<Option<i16>>),
    NullableInt32(Vec<Option<i32>>),
    NullableInt64(Vec<Option<i64>>),
    NullableUInt8(Vec<Option<u8>>),
    NullableUInt16(Vec<Option<u16>>),
    NullableUInt32(Vec<Option<u32>>),
    NullableUInt64(Vec<Option<u64>>),
    NullableFloat32(Vec<Option<f32>>),
    NullableFloat64(Vec<Option<f64>>),
    NullableText(Vec<Option<String>>),
}


pub fn is_same_columndata_type(col_1: &ColumnData, col_2: &ColumnData) -> bool {
    match col_1 {
        ColumnData::Int8(_) => {
            match col_2 {
                ColumnData::Int8(_) => true,
                _ => false
            }
        },
        ColumnData::Int16(_) => {
            match col_2 {
                ColumnData::Int16(_) => true,
                _ => false
            }
        },
        ColumnData::Int32(_) => {
            match col_2 {
                ColumnData::Int32(_) => true,
                _ => false
            }
        },
        ColumnData::Int64(_) => {
            match col_2 {
                ColumnData::Int64(_) => true,
                _ => false
            }
        },
        ColumnData::UInt8(_) => {
            match col_2 {
                ColumnData::UInt8(_) => true,
                _ => false
            }
        },
        ColumnData::UInt16(_) => {
            match col_2 {
                ColumnData::UInt16(_) => true,
                _ => false
            }
        },
        ColumnData::UInt32(_) => {
            match col_2 {
                ColumnData::UInt32(_) => true,
                _ => false
            }
        },
        ColumnData::UInt64(_) => {
            match col_2 {
                ColumnData::UInt64(_) => true,
                _ => false
            }
        },
        ColumnData::Float32(_) => {
            match col_2 {
                ColumnData::Float32(_) => true,
                _ => false
            }
        },
        ColumnData::Float64(_) => {
            match col_2 {
                ColumnData::Float64(_) => true,
                _ => false
            }
        },
        ColumnData::Text(_) => {
            match col_2 {
                ColumnData::Text(_) => true,
                _ => false
            }
        },
        ColumnData::NullableInt8(_) => {
            match col_2 {
                ColumnData::NullableInt8(_) => true,
                _ => false
            }
        },
        ColumnData::NullableInt16(_) => {
            match col_2 {
                ColumnData::NullableInt16(_) => true,
                _ => false
            }
        },
        ColumnData::NullableInt32(_) => {
            match col_2 {
                ColumnData::NullableInt32(_) => true,
                _ => false
            }
        },
        ColumnData::NullableInt64(_) => {
            match col_2 {
                ColumnData::NullableInt64(_) => true,
                _ => false
            }
        },
        ColumnData::NullableUInt8(_) => {
            match col_2 {
                ColumnData::NullableUInt8(_) => true,
                _ => false
            }
        },
        ColumnData::NullableUInt16(_) => {
            match col_2 {
                ColumnData::NullableUInt16(_) => true,
                _ => false
            }
        },
        ColumnData::NullableUInt32(_) => {
            match col_2 {
                ColumnData::NullableUInt32(_) => true,
                _ => false
            }
        },
        ColumnData::NullableUInt64(_) => {
            match col_2 {
                ColumnData::NullableUInt64(_) => true,
                _ => false
            }
        },
        ColumnData::NullableFloat32(_) => {
            match col_2 {
                ColumnData::NullableFloat32(_) => true,
                _ => false
            }
        },
        ColumnData::NullableFloat64(_) => {
            match col_2 {
                ColumnData::NullableFloat64(_) => true,
                _ => false
            }
        },
        ColumnData::NullableText(_) => {
            match col_2 {
                ColumnData::NullableText(_) => true,
                _ => false
            }
        },
    }
}
