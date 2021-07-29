use openapiv3::{OpenAPI, PathItem, ReferenceOr};
use psql::{errors::PSqlError, parser::Program};
use schemars::{schema_for, JsonSchema};
use sqlparser::dialect::MySqlDialect;
use std::{
    collections::HashMap,
    convert::Infallible,
    fs::File,
    io::Read,
    net::{SocketAddr, ToSocketAddrs},
    path::PathBuf,
};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

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
        File::open(&self.sql)
            .unwrap()
            .read_to_string(&mut sql_str)
            .unwrap();
        Program::parse(&dialect, &sql_str)
    }
}

#[derive(Debug, Clone)]
pub struct QueryWithProg {
    pub query: Query,
    pub prog: Program,
}

async fn run_http(plan: Plan, doc: OpenAPI) -> Result<(), ()> {
    use warp::Filter;
    async fn serve_doc(doc: OpenAPI) -> Result<impl warp::Reply, Infallible> {
        Ok(warp::reply::json(&doc))
    }
    async fn serve_query(qs: String, prog: Program) -> Result<impl warp::Reply, Infallible> {
        Ok(qs)
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
    let api_routes = queries
        .into_iter()
        .map(|(name, query)| {
            let prog = query.read_sql().unwrap();
            warp::get()
                .and(warp::path(plan.prefix.clone()))
                .and(warp::path(name))
                .and(warp::query::raw())
                .and(warp::any().map(move || prog.clone()))
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

#[tokio::main]
async fn main() -> Result<(), ()> {
    use std::env::args;

    pretty_env_logger::init();
    let plan_str = include_str!("plan.toml");
    let plan: Plan = toml::from_str(&plan_str).unwrap();
    let doc = plan.openapi_doc();
    if args().any(|arg| arg == "-s") {
        let schema = schema_for!(Plan);
        println!("{}", serde_json::to_string_pretty(&schema).unwrap());
        std::process::exit(0);
    } else if args().any(|arg| arg == "-o") {
        println!("{}", serde_json::to_string_pretty(&doc).unwrap());
        std::process::exit(0);
    }
    run_http(plan, doc).await
}