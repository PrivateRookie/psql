use crate::parser::{ParamValue, Program};
use futures::{future, lock::Mutex};
use openapiv3::OpenAPI;
use output::{QueryOutput, QueryOutputMapSer};
pub use plan::Plan;
use querystring::querify;
use serde::{Deserialize, Serialize};
use sqlparser::dialect::MySqlDialect;
use sqlx::{MySqlPool, SqlitePool};
use std::{collections::HashMap, convert::Infallible, hash::Hash, sync::Arc};
use warp::{
    hyper::{Method, StatusCode},
    Filter,
};

use self::plan::{PlanDb, Query};

mod index;
pub mod output;
pub mod plan;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiMsg {
    pub msg: String,
    pub code: u16,
}

async fn serve_doc(doc: OpenAPI) -> Result<impl warp::Reply, Infallible> {
    Ok(warp::reply::json(&doc))
}

async fn dynamic_doc(plan_db: PlanDb) -> Result<impl warp::Reply, Infallible> {
    let plan = plan_db.lock().await;
    Ok(warp::reply::json(&plan.openapi_doc()))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewQuery {
    /// query name
    pub name: String,
    /// connection string name
    pub conn: String,
    /// api summary
    pub summary: Option<String>,
    /// query sql or path starts with '@'
    pub sql: String,
    /// api relative url path
    pub path: String,
}

async fn add_query(
    new_queries: Vec<NewQuery>,
    plan_db: PlanDb,
) -> Result<impl warp::Reply, Infallible> {
    let mut plan = plan_db.lock().await;
    new_queries.into_iter().for_each(|new_query| {
        let NewQuery {
            name,
            conn,
            summary,
            sql,
            path,
        } = new_query;
        let query = Query {
            conn,
            summary,
            sql,
            path,
        };
        plan.queries.insert(name, query);
    });
    Ok(warp::reply::json(&{}))
}

fn get_context_from_body(
    body: &HashMap<String, ParamValue>,
    prog: &Program,
) -> Result<HashMap<String, ParamValue>, ApiMsg> {
    let mut context: HashMap<String, ParamValue> = HashMap::new();
    for p in prog.params.iter() {
        let found = body.get(&p.name);
        match (found, p.default.clone()) {
            (None, None) => {
                let code = warp::http::StatusCode::BAD_REQUEST;
                let msg = ApiMsg {
                    msg: format!("{} is required", p.name),
                    code: code.as_u16(),
                };
                return Err(msg);
            }
            (None, Some(default)) => {
                context.insert(p.name.clone(), default);
            }
            (Some(param), _) => match &p.ty {
                crate::parser::ParamTy::Basic(_) => match param {
                    ParamValue::Array(arr) => {
                        let code = warp::http::StatusCode::BAD_REQUEST;
                        let msg = ApiMsg {
                            msg: format!("{} expect single value, got {}", p.name, arr.len()),
                            code: code.as_u16(),
                        };
                        return Err(msg);
                    }
                    _ => {
                        context.insert(p.name.clone(), param.clone());
                    }
                },
                crate::parser::ParamTy::Array(_) => match param {
                    ParamValue::Array(_) => {
                        context.insert(p.name.clone(), param.clone());
                    }
                    _ => {
                        let code = warp::http::StatusCode::BAD_REQUEST;
                        let msg = ApiMsg {
                            msg: format!("{} expect array, got single value", p.name),
                            code: code.as_u16(),
                        };
                        return Err(msg);
                    }
                },
            },
        }
    }
    Ok(context)
}

fn get_context_from_qs(qs: String, prog: &Program) -> Result<HashMap<String, ParamValue>, ApiMsg> {
    let decoded = urlencoding::decode(&qs).unwrap();
    let qs_pairs = querify(&decoded);
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
                return Err(msg);
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
                        return Err(msg);
                    }
                    let raw_value = found.first().unwrap().1;
                    match ParamValue::from_arg_str(inner_ty, raw_value) {
                        Err(_) => {
                            let code = warp::http::StatusCode::BAD_REQUEST;
                            let msg = ApiMsg {
                                msg: format!("invalid value `{}` for {:?}", raw_value, inner_ty),
                                code: code.as_u16(),
                            };
                            return Err(msg);
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
                                return Err(msg);
                            }
                        }
                    }
                    context.insert(p.name.clone(), ParamValue::Array(parsed));
                }
            },
        }
    }
    Ok(context)
}

async fn serve_mysql_query(
    qs: String,
    prog: Program,
    pool: MySqlPool,
) -> Result<impl warp::Reply, Infallible> {
    let mut code = warp::http::StatusCode::BAD_REQUEST;
    match get_context_from_qs(qs, &prog) {
        Ok(context) => match prog.render(&MySqlDialect {}, &context) {
            Ok(stmts) => {
                if stmts.len() != 1 {
                    let msg = ApiMsg {
                        msg: format!("expect 1 sql statement, got {}", stmts.len()),
                        code: code.as_u16(),
                    };
                    return Ok(warp::reply::with_status(warp::reply::json(&msg), code));
                }
                let stmt = stmts.first().unwrap();
                match sqlx::query(&stmt.to_string())
                    .fetch_all(&pool)
                    .await
                    .map(|rows| QueryOutput { rows })
                {
                    Ok(output) => {
                        code = warp::http::StatusCode::OK;
                        let json = warp::reply::json(&QueryOutputMapSer(&output));
                        Ok(warp::reply::with_status(json, code))
                    }
                    Err(e) => {
                        let msg = ApiMsg {
                            msg: format!("SQL: {}\n{}", &stmt, e),
                            code: code.as_u16(),
                        };
                        Ok(warp::reply::with_status(warp::reply::json(&msg), code))
                    }
                }
            }
            Err(e) => {
                let msg = ApiMsg {
                    msg: format!("{:#?}", e),
                    code: code.as_u16(),
                };
                Ok(warp::reply::with_status(warp::reply::json(&msg), code))
            }
        },
        Err(msg) => Ok(warp::reply::with_status(
            warp::reply::json(&msg),
            StatusCode::from_u16(msg.code).unwrap(),
        )),
    }
}

async fn serve_sqlite_query(
    qs: String,
    prog: Program,
    pool: SqlitePool,
) -> Result<impl warp::Reply, Infallible> {
    let mut code = warp::http::StatusCode::BAD_REQUEST;
    match get_context_from_qs(qs, &prog) {
        Ok(context) => match prog.render(&MySqlDialect {}, &context) {
            Ok(stmts) => {
                if stmts.len() != 1 {
                    let msg = ApiMsg {
                        msg: format!("expect 1 sql statement, got {}", stmts.len()),
                        code: code.as_u16(),
                    };
                    return Ok(warp::reply::with_status(warp::reply::json(&msg), code));
                }
                let stmt = stmts.first().unwrap();
                match sqlx::query(&stmt.to_string())
                    .fetch_all(&pool)
                    .await
                    .map(|rows| QueryOutput { rows })
                {
                    Ok(output) => {
                        code = warp::http::StatusCode::OK;
                        let json = warp::reply::json(&QueryOutputMapSer(&output));
                        Ok(warp::reply::with_status(json, code))
                    }
                    Err(e) => {
                        let msg = ApiMsg {
                            msg: format!("SQL: {}\n{}", &stmt, e),
                            code: code.as_u16(),
                        };
                        Ok(warp::reply::with_status(warp::reply::json(&msg), code))
                    }
                }
            }
            Err(e) => {
                let msg = ApiMsg {
                    msg: format!("{:#?}", e),
                    code: code.as_u16(),
                };
                Ok(warp::reply::with_status(warp::reply::json(&msg), code))
            }
        },
        Err(msg) => Ok(warp::reply::with_status(
            warp::reply::json(&msg),
            StatusCode::from_u16(msg.code).unwrap(),
        )),
    }
}

fn new_query_body() -> impl Filter<Extract = (Vec<NewQuery>,), Error = warp::Rejection> + Clone {
    warp::body::content_length_limit(1024 * 16).and(warp::body::json())
}

fn maybe_empty_body(
) -> impl Filter<Extract = (HashMap<String, ParamValue>,), Error = warp::Rejection> + Clone {
    warp::body::json()
}

async fn serve_with_context(
    prog: &Program,
    plan_db: PlanDb,
    query: &Query,
    code: &mut warp::http::StatusCode,
    context: HashMap<String, ParamValue>,
    mysql_dbs: Arc<Mutex<HashMap<String, MySqlPool>>>,
    sqlite_dbs: Arc<Mutex<HashMap<String, SqlitePool>>>,
) -> Result<warp::reply::WithStatus<warp::reply::Json>, Infallible> {
    match prog.render(&MySqlDialect {}, &context) {
        Ok(stmts) => {
            if stmts.len() != 1 {
                let msg = ApiMsg {
                    msg: format!("expect 1 sql statement, got {}", stmts.len()),
                    code: code.as_u16(),
                };
                return Ok(warp::reply::with_status(
                    warp::reply::json(&msg),
                    code.clone(),
                ));
            }
            let stmt = stmts.first().unwrap();
            match mysql_dbs.lock().await.get(&query.conn) {
                Some(pool) => {
                    match sqlx::query(&stmt.to_string())
                        .fetch_all(pool)
                        .await
                        .map(|rows| QueryOutput { rows })
                    {
                        Ok(output) => {
                            let code = warp::http::StatusCode::OK;
                            let json = warp::reply::json(&QueryOutputMapSer(&output));
                            Ok(warp::reply::with_status(json, code))
                        }
                        Err(e) => {
                            let msg = ApiMsg {
                                msg: format!("SQL: {}\n{}", &stmt, e),
                                code: code.as_u16(),
                            };
                            Ok(warp::reply::with_status(
                                warp::reply::json(&msg),
                                code.clone(),
                            ))
                        }
                    }
                }
                None => {
                    let dbs = sqlite_dbs.lock().await;
                    let pool = dbs.get(&query.conn).unwrap();
                    match sqlx::query(&stmt.to_string())
                        .fetch_all(pool)
                        .await
                        .map(|rows| QueryOutput { rows })
                    {
                        Ok(output) => {
                            let code = warp::http::StatusCode::OK;
                            let json = warp::reply::json(&QueryOutputMapSer(&output));
                            Ok(warp::reply::with_status(json, code))
                        }
                        Err(e) => {
                            let msg = ApiMsg {
                                msg: format!("SQL: {}\n{}", &stmt, e),
                                code: code.as_u16(),
                            };
                            Ok(warp::reply::with_status(
                                warp::reply::json(&msg),
                                code.clone(),
                            ))
                        }
                    }
                }
            }
        }
        Err(e) => {
            let msg = ApiMsg {
                msg: format!("{:#?}", e),
                code: code.as_u16(),
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&msg),
                code.clone(),
            ))
        }
    }
}

async fn serve_query(
    method: Method,
    qs: String,
    path: warp::path::FullPath,
    json_body: HashMap<String, ParamValue>,
    plan_db: PlanDb,
    mysql_dbs: Arc<Mutex<HashMap<String, MySqlPool>>>,
    sqlite_dbs: Arc<Mutex<HashMap<String, SqlitePool>>>,
) -> Result<impl warp::Reply, Infallible> {
    let plan = plan_db.lock().await;
    let all_paths: Vec<(String, Query)> = plan
        .queries
        .values()
        .map(|q| (q.path.clone(), q.clone()))
        .collect();
    match all_paths.iter().position(|p| path.as_str().ends_with(&p.0)) {
        Some(idx) => {
            let query = &all_paths.get(idx).unwrap().1;
            let prog = query.read_sql().unwrap();
            let mut code = warp::http::StatusCode::BAD_REQUEST;
            let may_be_context = match method {
                Method::POST | Method::PUT | Method::DELETE => {
                    get_context_from_body(&json_body, &prog)
                }
                _ => get_context_from_qs(qs, &prog),
            };
            match may_be_context {
                Ok(context) => {
                    serve_with_context(
                        &prog,
                        plan_db.clone(),
                        query,
                        &mut code,
                        context,
                        mysql_dbs,
                        sqlite_dbs,
                    )
                    .await
                }
                Err(msg) => Ok(warp::reply::with_status(
                    warp::reply::json(&msg),
                    StatusCode::from_u16(msg.code).unwrap(),
                )),
            }
        }
        None => {
            let status = warp::http::StatusCode::BAD_REQUEST;
            let msg = ApiMsg {
                msg: format!("{} not found", path.as_str()),
                code: 404,
            };
            Ok(warp::reply::with_status(warp::reply::json(&msg), status))
        }
    }
}

pub async fn run_dynamic_http(
    plan: Plan,
    mysql_conns: HashMap<String, sqlx::MySqlPool>,
    sqlite_conns: HashMap<String, sqlx::SqlitePool>,
) -> Result<(), ()> {
    let prefix = plan.prefix.clone();
    let query_prefix = prefix.clone();
    let doc_path = plan.doc_path.clone();
    let mysql_dbs = Arc::new(Mutex::new(mysql_conns));
    let sqlite_dbs = Arc::new(Mutex::new(sqlite_conns));
    let plan_db = Arc::new(Mutex::new(plan.clone()));
    let plan_doc = plan_db.clone();
    let doc_route = warp::get()
        .and(warp::path(prefix.clone()))
        .and(warp::path(plan.doc_path.clone()))
        .and(warp::any().map(move || plan_doc.clone()))
        .and_then(dynamic_doc);
    let index = warp::get()
        .and(warp::path("index"))
        .and(warp::any().map(move || format!("{}/{}", &prefix.clone(), &doc_path)))
        .and_then(index::serve_index);
    let plan_c = plan_db.clone();
    let add_query_route = warp::post()
        .and(warp::path(query_prefix.clone()))
        .and(warp::path("add_query"))
        .and(new_query_body())
        .and(warp::any().map(move || plan_db.clone()))
        .and_then(add_query);
    let query_route = warp::any()
        .and(warp::method())
        .and(warp::query::raw().or(warp::any().map(String::new)).unify())
        .and(warp::path::full())
        .and(
            warp::body::json()
                .or(warp::any().map(|| HashMap::default()))
                .unify(),
        )
        .and(warp::any().map(move || plan_c.clone()))
        .and(warp::any().map(move || mysql_dbs.clone()))
        .and(warp::any().map(move || sqlite_dbs.clone()))
        .and_then(serve_query);
    let fs = plan
        .address
        .iter()
        .map(move |addr| {
            warp::serve(
                index
                    .clone()
                    .or(doc_route.clone())
                    .or(add_query_route.clone())
                    .or(query_route.clone()),
            )
            .bind_ephemeral((addr.ip(), addr.port()))
            .1
        })
        .collect::<Vec<_>>();
    future::join_all(fs).await;
    Ok(())
}

pub async fn run_http(
    plan: Plan,
    doc: OpenAPI,
    mysql_conns: HashMap<String, sqlx::MySqlPool>,
    sqlite_conns: HashMap<String, sqlx::SqlitePool>,
) -> Result<(), ()> {
    let prefix = plan.prefix.clone();
    let doc_path = plan.doc_path.clone();
    let doc_route = warp::get()
        .and(warp::path(prefix.clone()))
        .and(warp::path(plan.doc_path.clone()))
        .and(warp::any().map(move || doc.clone()))
        .and_then(serve_doc);
    let index = warp::get()
        .and(warp::path("index"))
        .and(warp::any().map(move || format!("{}/{}", &prefix, &doc_path)))
        .and_then(index::serve_index);
    let queries = plan.queries.clone();
    let prefix = plan.prefix.clone();
    let prefix_cloned = plan.prefix.clone();
    let mysql_api_routes = queries
        .clone()
        .into_iter()
        .filter(|(_name, query)| plan.mysql_conns.get(&query.conn).is_some())
        .map(move |(_name, query)| {
            let pool = mysql_conns.get(&query.conn).cloned().unwrap();
            let prog = query.read_sql().unwrap();
            warp::get()
                .and(warp::path(prefix.clone()))
                .and(warp::path(query.path))
                .and(warp::query::raw().or(warp::any().map(String::new)).unify())
                .and(warp::any().map(move || prog.clone()))
                .and(warp::any().map(move || pool.clone()))
                .and_then(serve_mysql_query)
                .boxed()
        })
        .reduce(|pre, next| pre.or(next).unify().boxed());
    let sqlite_api_routes = queries
        .clone()
        .into_iter()
        .filter(|(_name, query)| plan.sqlite_conns.get(&query.conn).is_some())
        .map(move |(_name, query)| {
            let pool = sqlite_conns.get(&query.conn).cloned().unwrap();
            let prog = query.read_sql().unwrap();
            warp::get()
                .and(warp::path(prefix_cloned.clone()))
                .and(warp::path(query.path))
                .and(warp::query::raw().or(warp::any().map(String::new)).unify())
                .and(warp::any().map(move || prog.clone()))
                .and(warp::any().map(move || pool.clone()))
                .and_then(serve_sqlite_query)
                .boxed()
        })
        .reduce(|pre, next| pre.or(next).unify().boxed());
    match (mysql_api_routes, sqlite_api_routes) {
        (None, None) => {
            let fs = plan
                .address
                .iter()
                .map(move |addr| {
                    warp::serve(index.clone().or(doc_route.clone()))
                        .bind_ephemeral((addr.ip(), addr.port()))
                        .1
                })
                .collect::<Vec<_>>();

            future::join_all(fs).await;
        }
        (None, Some(routes)) => {
            let fs = plan
                .address
                .iter()
                .map(move |addr| {
                    warp::serve(index.clone().or(doc_route.clone()).or(routes.clone()))
                        .bind_ephemeral((addr.ip(), addr.port()))
                        .1
                })
                .collect::<Vec<_>>();
            future::join_all(fs).await;
        }
        (Some(routes), None) => {
            let fs = plan
                .address
                .iter()
                .map(move |addr| {
                    warp::serve(index.clone().or(doc_route.clone()).or(routes.clone()))
                        .bind_ephemeral((addr.ip(), addr.port()))
                        .1
                })
                .collect::<Vec<_>>();
            future::join_all(fs).await;
        }
        (Some(mysql_api), Some(sqlite_api)) => {
            let fs = plan
                .address
                .iter()
                .map(move |addr| {
                    warp::serve(
                        index
                            .clone()
                            .or(doc_route.clone())
                            .or(mysql_api.clone())
                            .or(sqlite_api.clone()),
                    )
                    .bind_ephemeral((addr.ip(), addr.port()))
                    .1
                })
                .collect::<Vec<_>>();
            future::join_all(fs).await;
        }
    }

    Ok(())
}
