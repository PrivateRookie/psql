use crate::{
    errors::PSqlError,
    parser::{ParamValue, Program},
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use openapiv3::{OpenAPI, PathItem, ReferenceOr};
use querystring::querify;
use schemars::JsonSchema;
use sqlparser::dialect::MySqlDialect;
use sqlx::{
    mysql::{MySqlColumn, MySqlRow, MySqlValueRef},
    types::time::{Date, Time},
    Column, Row, TypeInfo, Value, ValueRef,
};
use std::{
    collections::HashMap,
    convert::Infallible,
    fs::File,
    io::Read,
    net::{SocketAddr, ToSocketAddrs},
    path::PathBuf,
};

use indexmap::IndexMap;
use serde::{
    ser::{SerializeMap, SerializeSeq},
    Deserialize, Serialize,
};

fn default_prefix() -> String {
    "api".to_string()
}

fn default_addr() -> Vec<SocketAddr> {
    "127.0.0.1:12345".to_socket_addrs().unwrap().collect()
}

/// http serve config
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Plan {
    /// doc title
    pub title: String,
    /// doc description
    pub description: Option<String>,
    /// api contact info
    pub contact: Option<Contact>,
    /// http service bind address
    #[serde(default = "default_addr")]
    pub address: Vec<SocketAddr>,
    /// api prefix route
    #[serde(default = "default_prefix")]
    pub prefix: String,
    /// database connections
    pub conns: HashMap<String, String>,
    /// api paths
    pub queries: IndexMap<String, Query>,
}

impl Plan {
    pub fn to_warp_api(&self) {
        todo!()
    }

    pub async fn create_connections(&self) -> Result<HashMap<String, sqlx::MySqlPool>, String> {
        let mut pools = HashMap::new();
        for (name, uri) in self.conns.iter() {
            match sqlx::MySqlPool::connect(uri).await {
                Ok(pool) => {
                    pools.insert(name.clone(), pool);
                }
                Err(e) => {
                    return Err(e.to_string());
                }
            }
        }
        Ok(pools)
    }

    /// pub generate api doc
    pub fn openapi_doc(&self) -> OpenAPI {
        let Self {
            title,
            description,
            contact,
            ..
        } = self.clone();
        let contact = contact.map(|c| {
            let Contact { name, url, email } = c;
            openapiv3::Contact {
                name: Some(name),
                url: url,
                email: email,
                extensions: Default::default(),
            }
        });
        let info = openapiv3::Info {
            title,
            description,
            contact,
            ..Default::default()
        };
        let server = openapiv3::Server {
            url: self.prefix.clone(),
            ..Default::default()
        };
        let mut paths = IndexMap::new();
        self.queries.clone().into_iter().for_each(|(name, query)| {
            let prog = query.read_sql().unwrap();
            let Query { summary, .. } = query;
            let get_op = openapiv3::Operation {
                summary,
                parameters: prog.generate_openapi(),
                responses: openapiv3::Responses {
                    default: Some(ReferenceOr::Item(openapiv3::Response {
                        description: "default response".to_string(),
                        headers: IndexMap::default(),
                        ..Default::default()
                    })),
                    responses: Default::default(),
                },
                ..Default::default()
            };
            paths.insert(
                format!("/{}", name),
                ReferenceOr::Item(PathItem {
                    get: Some(get_op),
                    ..Default::default()
                }),
            );
        });
        OpenAPI {
            info,
            openapi: "3.0.0".to_string(),
            servers: vec![server],
            paths,
            ..Default::default()
        }
    }
}

/// doc contact info
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Contact {
    pub name: String,
    pub url: Option<String>,
    pub email: Option<String>,
}

/// api query description
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Query {
    /// connection string name
    pub conn: String,
    /// api summary
    pub summary: Option<String>,
    /// sql file location
    pub sql: PathBuf,
    /// api relative url path
    pub path: String,
}

impl Query {
    pub fn read_sql(&self) -> Result<Program, PSqlError> {
        let mut sql_str = String::new();
        let dialect = MySqlDialect {};
        let mut file = File::open(&self.sql)
            .map_err(|e| PSqlError::ReadSQLError(self.sql.clone(), e.to_string()))?;
        file.read_to_string(&mut sql_str)
            .map_err(|e| PSqlError::ReadSQLError(self.sql.clone(), e.to_string()))?;
        Program::parse(&dialect, &sql_str)
    }
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiMsg {
    pub msg: String,
    pub code: u16,
}

pub async fn run_http(
    plan: Plan,
    doc: OpenAPI,
    conns: HashMap<String, sqlx::MySqlPool>,
) -> Result<(), ()> {
    use warp::Filter;
    async fn serve_doc(doc: OpenAPI) -> Result<impl warp::Reply, Infallible> {
        Ok(warp::reply::json(&doc))
    }
    async fn serve_query(
        qs: String,
        prog: Program,
        pool: sqlx::MySqlPool,
    ) -> Result<impl warp::Reply, Infallible> {
        let qs_pairs = querify(&qs);
        let mut context: HashMap<String, ParamValue> = HashMap::new();
        for p in prog.params.iter() {
            let found = qs_pairs
                .iter()
                .filter(|(k, _)| *k == p.name)
                .collect::<Vec<&(&str, &str)>>();
            match (found.is_empty(), p.default.clone()) {
                (true, None) => {
                    let code = warp::http::StatusCode::BAD_REQUEST;
                    let msg = ApiMsg {
                        msg: format!("{} is required", p.name),
                        code: code.as_u16(),
                    };
                    return Ok(warp::reply::with_status(warp::reply::json(&msg), code));
                }
                (true, Some(default)) => {
                    context.insert(p.name.clone(), default);
                }
                (false, _) => match &p.ty {
                    crate::parser::ParamTy::Basic(inner_ty) => {
                        if found.len() > 1 {
                            let code = warp::http::StatusCode::BAD_REQUEST;
                            let msg = ApiMsg {
                                msg: format!("{} expect single value, got {}", p.name, found.len()),
                                code: code.as_u16(),
                            };
                            return Ok(warp::reply::with_status(warp::reply::json(&msg), code));
                        }
                        let raw_value = found.first().unwrap().1;
                        match ParamValue::from_arg_str(inner_ty, raw_value) {
                            Err(_) => {
                                let code = warp::http::StatusCode::BAD_REQUEST;
                                let msg = ApiMsg {
                                    msg: format!(
                                        "invalid value `{}` for {:?}",
                                        raw_value, inner_ty
                                    ),
                                    code: code.as_u16(),
                                };
                                return Ok(warp::reply::with_status(warp::reply::json(&msg), code));
                            }
                            Ok(val) => {
                                context.insert(p.name.clone(), val);
                            }
                        }
                    }
                    crate::parser::ParamTy::Array(inner_ty) => {
                        let mut parsed = vec![];
                        for (_, raw) in found {
                            match ParamValue::from_arg_str(inner_ty, raw) {
                                Ok(val) => parsed.push(val),
                                Err(_) => {
                                    let code = warp::http::StatusCode::BAD_REQUEST;
                                    let msg = ApiMsg {
                                        msg: format!("invalid value `{}` for {:?}", raw, inner_ty),
                                        code: code.as_u16(),
                                    };
                                    return Ok(warp::reply::with_status(
                                        warp::reply::json(&msg),
                                        code,
                                    ));
                                }
                            }
                        }
                        context.insert(p.name.clone(), ParamValue::Array(parsed));
                    }
                },
            }
        }
        let renderd: String = prog
            .render(&MySqlDialect {}, &context)
            .unwrap()
            .iter()
            .map(|stmt| stmt.to_string())
            .collect();
        let code = warp::http::StatusCode::OK;
        let result = sqlx::query(&renderd)
            .fetch_all(&pool)
            .await
            .map(|rows| QueryOutput { rows })
            .unwrap();
        Ok(warp::reply::with_status(
            warp::reply::json(&QueryOutputMapSer(&result)),
            code,
        ))
    }
    async fn index() -> Result<impl warp::Reply, Infallible> {
        Ok("<h1>hello</h1>".to_string())
    }
    let doc_route = warp::get()
        .and(warp::path("doc"))
        .and(warp::any().map(move || doc.clone()))
        .and_then(serve_doc);
    let index = warp::get().and(warp::path("index")).and_then(index);
    let queries = plan.queries.clone();
    let prefix = plan.prefix.clone();
    let api_routes = queries
        .into_iter()
        .map(move |(name, query)| {
            let pool = conns.get(&name).map(|p| p.clone()).unwrap();
            let prog = query.read_sql().unwrap();
            warp::get()
                .and(warp::path(prefix.clone()))
                .and(warp::path(name))
                .and(warp::query::raw())
                .and(warp::any().map(move || prog.clone()))
                .and(warp::any().map(move || pool.clone()))
                .and_then(serve_query)
                .boxed()
        })
        .reduce(|pre, next| pre.or(next).unify().boxed())
        .unwrap();

    let addr = plan.address.first().unwrap();
    warp::serve(index.or(doc_route).or(api_routes))
        .run((addr.ip(), addr.port()))
        .await;
    Ok(())
}
