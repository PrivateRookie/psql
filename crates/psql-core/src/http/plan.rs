use futures::lock::Mutex;
use indexmap::IndexMap;
use openapiv3::{OpenAPI, PathItem, ReferenceOr};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlparser::dialect::MySqlDialect;
use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
};

use crate::{errors::PSqlError, parser::Program};

fn default_prefix() -> String {
    "api".to_string()
}

fn default_addr() -> Vec<SocketAddr> {
    "127.0.0.1:12345".to_socket_addrs().unwrap().collect()
}

fn default_doc_path() -> String {
    "_doc".to_string()
}

pub type PlanDb = Arc<Mutex<Plan>>;

/// http serve config
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Plan {
    /// doc title
    pub title: String,
    /// doc description
    pub description: Option<String>,
    /// api contact info
    pub contact: Option<Contact>,
    /// swagger api doc path
    #[serde(default = "default_doc_path")]
    pub doc_path: String,
    /// http service bind address
    #[serde(default = "default_addr")]
    pub address: Vec<SocketAddr>,
    /// api prefix route
    #[serde(default = "default_prefix")]
    pub prefix: String,
    /// database connections
    #[serde(default)]
    pub sqlite_conns: HashMap<String, String>,
    /// database mysql connections
    #[serde(default)]
    pub mysql_conns: HashMap<String, String>,
    /// api paths
    #[serde(default)]
    pub queries: IndexMap<String, Query>,
}

impl Plan {
    pub fn to_warp_api(&self) {
        todo!()
    }

    pub async fn create_connections(
        &self,
    ) -> Result<
        (
            HashMap<String, sqlx::MySqlPool>,
            HashMap<String, sqlx::SqlitePool>,
        ),
        String,
    > {
        let mut mysql_pools = HashMap::new();
        for (name, uri) in self.mysql_conns.iter() {
            match sqlx::MySqlPool::connect(uri).await {
                Ok(pool) => {
                    mysql_pools.insert(name.clone(), pool);
                }
                Err(e) => {
                    return Err(e.to_string());
                }
            }
        }
        let mut sqlite_pools = HashMap::new();
        for (name, uri) in self.sqlite_conns.iter() {
            match sqlx::SqlitePool::connect(uri).await {
                Ok(pool) => {
                    sqlite_pools.insert(name.clone(), pool);
                }
                Err(e) => {
                    return Err(e.to_string());
                }
            }
        }
        Ok((mysql_pools, sqlite_pools))
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
                url,
                email,
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
            url: format!("/{}", self.prefix),
            ..Default::default()
        };
        let mut paths = IndexMap::new();
        self.queries.clone().into_iter().for_each(|(_, query)| {
            let prog = query.read_sql().unwrap();
            let Query { summary, tags, .. } = query;
            let mut operation = openapiv3::Operation {
                summary,
                tags,
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
            let val = match query.method {
                Method::Get => {
                    operation.parameters = prog.generate_params();
                    ReferenceOr::Item(PathItem {
                        get: Some(operation),
                        ..Default::default()
                    })
                }
                Method::Post => {
                    operation.request_body = prog.generate_req_body();
                    ReferenceOr::Item(PathItem {
                        post: Some(operation),
                        ..Default::default()
                    })
                }
                Method::Put => {
                    operation.request_body = prog.generate_req_body();
                    ReferenceOr::Item(PathItem {
                        put: Some(operation),
                        ..Default::default()
                    })
                }
                Method::Patch => {
                    operation.request_body = prog.generate_req_body();
                    ReferenceOr::Item(PathItem {
                        patch: Some(operation),
                        ..Default::default()
                    })
                }
                Method::Delete => {
                    operation.request_body = prog.generate_req_body();
                    ReferenceOr::Item(PathItem {
                        delete: Some(operation),
                        ..Default::default()
                    })
                }
            };
            paths.insert(format!("/{}", query.path), val);
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum Dialect {
    #[serde(rename = "mysql")]
    Mysql,
    #[serde(rename = "sqlite")]
    Sqlite,
}

impl Dialect {
    pub fn from_uri(uri: &str) -> Self {
        if uri.starts_with("mysql") {
            Self::Mysql
        } else {
            Self::Sqlite
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum Method {
    #[serde(rename = "GET")]
    Get,
    #[serde(rename = "POST")]
    Post,
    #[serde(rename = "PUT")]
    Put,
    #[serde(rename = "PATCH")]
    Patch,
    #[serde(rename = "DELETE")]
    Delete,
}

impl From<Method> for warp::http::Method {
    fn from(m: Method) -> Self {
        match m {
            Method::Get => warp::http::Method::GET,
            Method::Post => warp::http::Method::POST,
            Method::Put => warp::http::Method::PUT,
            Method::Patch => warp::http::Method::PATCH,
            Method::Delete => warp::http::Method::DELETE,
        }
    }
}

impl Default for Method {
    fn default() -> Self {
        Self::Get
    }
}

/// api query description
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Query {
    /// connection string name
    pub conn: String,
    /// http method
    #[serde(default)]
    pub method: Method,
    /// api summary
    pub summary: Option<String>,
    /// query sql or path starts with '@'
    pub sql: String,
    /// api relative url path
    pub path: String,
    /// api tags
    #[serde(default)]
    pub tags: Vec<String>,
}

impl Query {
    pub fn read_sql(&self) -> Result<Program, PSqlError> {
        let sql_str = if self.sql.starts_with('@') {
            let path = self.sql.trim_start_matches('@');
            let mut sql_str = String::new();

            let mut file = File::open(&path)
                .map_err(|e| PSqlError::ReadSQLError(self.sql.clone(), e.to_string()))?;
            file.read_to_string(&mut sql_str)
                .map_err(|e| PSqlError::ReadSQLError(self.sql.clone(), e.to_string()))?;
            sql_str
        } else {
            self.sql.clone()
        };
        let dialect = MySqlDialect {};
        Program::parse(&dialect, &sql_str)
    }
}
