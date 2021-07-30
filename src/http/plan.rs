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
    path::PathBuf,
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
            url: format!("/{}", self.prefix),
            ..Default::default()
        };
        let mut paths = IndexMap::new();
        self.queries.clone().into_iter().for_each(|(_, query)| {
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
                format!("/{}", query.path),
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
