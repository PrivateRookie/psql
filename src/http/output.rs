use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Serialize, ser::{SerializeMap, SerializeSeq}};
use sqlx::{
    mysql::{MySqlColumn, MySqlRow, MySqlValueRef},
    types::time::{Date, Time},
    Column, Row, TypeInfo, Value, ValueRef,
};
pub struct QueryOutput {
    pub rows: Vec<MySqlRow>,
}
pub struct PSqlColumn<'a> {
    pub col: &'a MySqlColumn,
    pub val_ref: MySqlValueRef<'a>,
}

pub struct QueryOutputMapSer<'a>(pub &'a QueryOutput);
struct PSqlRowMapSer<'a>(&'a MySqlRow);
struct QueryOutputListSer<'a>(&'a QueryOutput);
struct PSqlRowListSer<'a>(&'a MySqlRow);

impl<'a> Serialize for QueryOutputMapSer<'a> {
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

impl<'a> Serialize for PSqlRowMapSer<'a> {
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

impl<'a> Serialize for QueryOutputListSer<'a> {
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

impl<'a> Serialize for PSqlRowListSer<'a> {
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

impl<'a> Serialize for PSqlColumn<'a> {
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
                t => unreachable!(t),
            }
        }
    }
}
