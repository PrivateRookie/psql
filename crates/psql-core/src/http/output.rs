use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{
    ser::{SerializeMap, SerializeSeq},
    Serialize,
};
use sqlx::{
    mysql::{MySqlColumn, MySqlRow, MySqlValueRef},
    sqlite::{SqliteColumn, SqliteRow, SqliteValueRef},
    types::time::{Date, Time},
    Column, Row, TypeInfo, Value, ValueRef,
};
pub struct QueryOutput<R: Row> {
    pub rows: Vec<R>,
}
pub struct PSqlColumn<'a, C: Column, V: ValueRef<'a>> {
    pub col: &'a C,
    pub val_ref: V,
}

pub struct QueryOutputMapSer<'a, R: Row>(pub &'a QueryOutput<R>);
struct PSqlRowMapSer<'a, R: Row>(&'a R);
struct QueryOutputListSer<'a, R: Row>(&'a QueryOutput<R>);
struct PSqlRowListSer<'a, R: Row>(&'a R);

macro_rules! impl_query_output_map_ser {
    ($row:ident) => {
        impl<'a> Serialize for QueryOutputMapSer<'a, $row> {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                let mut seq = serializer.serialize_seq(Some(self.0.rows.len()))?;
                for row in self.0.rows.iter().map(PSqlRowMapSer) {
                    seq.serialize_element(&row)?;
                }
                seq.end()
            }
        }
    };
}

impl_query_output_map_ser!(MySqlRow);
impl_query_output_map_ser!(SqliteRow);

macro_rules! impl_row_map_ser {
    ($row:ident) => {
        impl<'a> Serialize for PSqlRowMapSer<'a, $row> {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                let mut map = serializer.serialize_map(Some(self.0.len()))?;
                for col in self.0.columns().iter().map(|c| {
                    let val_ref = self.0.try_get_raw(c.ordinal()).unwrap();
                    PSqlColumn { col: c, val_ref }
                }) {
                    map.serialize_entry(col.col.name(), &col)?;
                }
                map.end()
            }
        }
    };
}

impl_row_map_ser!(MySqlRow);
impl_row_map_ser!(SqliteRow);

macro_rules! impl_query_output_list_ser {
    ($row:ident) => {
        impl<'a> Serialize for QueryOutputListSer<'a, $row> {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                let mut seq = serializer.serialize_seq(Some(self.0.rows.len()))?;
                for row in self.0.rows.iter().map(PSqlRowListSer) {
                    seq.serialize_element(&row)?;
                }
                seq.end()
            }
        }
    };
}

impl_query_output_list_ser!(MySqlRow);
impl_query_output_list_ser!(SqliteRow);

macro_rules! impl_row_list_ser {
    ($row:ident) => {
        impl<'a> Serialize for PSqlRowListSer<'a, $row> {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
                for col in self.0.columns().iter().map(|c| {
                    let val_ref = self.0.try_get_raw(c.ordinal()).unwrap();
                    PSqlColumn { col: c, val_ref }
                }) {
                    seq.serialize_element(&col)?;
                }
                seq.end()
            }
        }
    };
}

impl_row_list_ser!(MySqlRow);
impl_row_list_ser!(SqliteRow);

impl<'a> Serialize for PSqlColumn<'a, MySqlColumn, MySqlValueRef<'a>> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let val = ValueRef::to_owned(&self.val_ref);
        if val.is_null() {
            serializer.serialize_none()
        } else {
            match val.type_info().name() {
                "BOOLEAN" => {
                    let v = val.try_decode::<bool>().unwrap();
                    serializer.serialize_bool(v)
                }
                "TINYINT UNSIGNED" | "SMALLINT UNSIGNED" | "INT UNSIGNED"
                | "MEDIUMINT UNSIGNED" | "BIGINT UNSIGNED" => {
                    let v = val.try_decode::<u64>().unwrap();
                    serializer.serialize_u64(v)
                }
                "TINYINT" | "SMALLINT" | "INT" | "MEDIUMINT" | "BIGINT" => {
                    let v = val.try_decode::<i64>().unwrap();
                    serializer.serialize_i64(v)
                }
                "FLOAT" => {
                    let v = val.try_decode::<f32>().unwrap();
                    serializer.serialize_f32(v)
                }
                "DOUBLE" => {
                    let v = val.try_decode::<f64>().unwrap();
                    serializer.serialize_f64(v)
                }
                "NULL" => serializer.serialize_none(),
                "DATE" => {
                    let v = val.try_decode::<Date>().unwrap();
                    serializer.serialize_str(&v.to_string())
                }
                "TIME" => {
                    let v = val.try_decode::<Time>().unwrap();
                    serializer.serialize_str(&v.to_string())
                }
                "YEAR" => {
                    let v = val.try_decode::<u64>().unwrap();
                    serializer.serialize_u64(v)
                }
                // NOTE not sure for this
                // ref https://dev.mysql.com/doc/refman/8.0/en/time-zone-support.html
                "DATETIME" => {
                    let v = val
                        .try_decode::<sqlx::types::time::OffsetDateTime>()
                        .unwrap();
                    serializer.serialize_str(&v.to_string())
                }
                "TIMESTAMP" => {
                    let v = val.try_decode::<DateTime<Utc>>().unwrap();
                    serializer.serialize_str(&v.to_string())
                }
                "BIT" | "ENUM" | "SET" => {
                    let v = val.try_decode::<String>().unwrap();
                    serializer.serialize_str(&v)
                }
                "DECIMAL" => {
                    let v = val.try_decode::<BigDecimal>().unwrap();
                    serializer.serialize_str(&v.to_string())
                }
                "GEOMETRY" | "JSON" => {
                    let v = val.try_decode::<String>().unwrap();
                    serializer.serialize_str(&v)
                }
                "CHAR" | "VARCHAR" | "TINYTEXT" | "TEXT" | "MEDIUMTEXT" | "LONGTEXT" => {
                    let v = val.try_decode::<String>().unwrap();
                    serializer.serialize_str(&v)
                }
                "TINYBLOB" | "BLOB" | "MEDIUMBLOB" | "LONGBLOB" | "BINARY" | "VARBINARY" => {
                    let b64_str = val.try_decode::<Vec<u8>>().map(base64::encode).unwrap();
                    serializer.serialize_str(&b64_str)
                }
                t => unreachable!("{}", t),
            }
        }
    }
}

impl<'a> Serialize for PSqlColumn<'a, SqliteColumn, SqliteValueRef<'a>> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let val = ValueRef::to_owned(&self.val_ref);
        if val.is_null() {
            serializer.serialize_none()
        } else {
            match val.type_info().name() {
                "NULL" => serializer.serialize_none(),
                "TEXT" => {
                    let v = val.try_decode::<String>().unwrap();
                    serializer.serialize_str(&v)
                }
                "REAL" => {
                    let v = val.try_decode::<f64>().unwrap();
                    serializer.serialize_f64(v)
                }
                "BLOB" => {
                    let b64_str = val.try_decode::<Vec<u8>>().map(base64::encode).unwrap();
                    serializer.serialize_str(&b64_str)
                }
                "INTEGER" => {
                    let v = val.try_decode::<i64>().unwrap();
                    serializer.serialize_i64(v)
                }
                "NUMERIC" => {
                    let v = val.try_decode::<String>().unwrap();
                    serializer.serialize_str(&v)
                }
                "BOOLEAN" => {
                    let v = val.try_decode::<bool>().unwrap();
                    serializer.serialize_bool(v)
                }
                "DATE" => {
                    let v = val.try_decode::<String>().unwrap();
                    serializer.serialize_str(&v)
                }
                "TIME" => {
                    let v = val.try_decode::<String>().unwrap();
                    serializer.serialize_str(&v)
                }
                "DATETIME" => {
                    let v = val.try_decode::<String>().unwrap();
                    serializer.serialize_str(&v)
                }

                t => unreachable!("{}", t),
            }
        }
    }
}
