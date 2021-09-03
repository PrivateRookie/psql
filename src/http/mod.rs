use crate::parser::{ParamValue, Program};
use futures::future;
use openapiv3::OpenAPI;
use output::{QueryOutput, QueryOutputMapSer};
pub use plan::Plan;
use querystring::querify;
use serde::{Deserialize, Serialize};
use sqlparser::dialect::MySqlDialect;
use sqlx::{MySqlPool, SqlitePool};
use std::{collections::HashMap, convert::Infallible};
use warp::{hyper::StatusCode, Filter};

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

fn get_context(qs: String, prog: &Program) -> Result<HashMap<String, ParamValue>, ApiMsg> {
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
    match get_context(qs, &prog) {
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
                return Ok(warp::reply::with_status(warp::reply::json(&msg), code));
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
    match get_context(qs, &prog) {
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
                return Ok(warp::reply::with_status(warp::reply::json(&msg), code));
            }
        },
        Err(msg) => Ok(warp::reply::with_status(
            warp::reply::json(&msg),
            StatusCode::from_u16(msg.code).unwrap(),
        )),
    }
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
        .filter(|(_name, query)| match query.conn.0 {
            plan::Dialect::Mysql => true,
            plan::Dialect::Sqlite => false,
        })
        .map(move |(_name, query)| {
            let pool = mysql_conns.get(&query.conn.1).map(|p| p.clone()).unwrap();
            let prog = query.read_sql().unwrap();
            warp::get()
                .and(warp::path(prefix.clone()))
                .and(warp::path(query.path))
                .and(
                    warp::query::raw()
                        .or(warp::any().map(|| String::new()))
                        .unify(),
                )
                .and(warp::any().map(move || prog.clone()))
                .and(warp::any().map(move || pool.clone()))
                .and_then(serve_mysql_query)
                .boxed()
        })
        .reduce(|pre, next| pre.or(next).unify().boxed());
    let sqlite_api_routes = queries
        .clone()
        .into_iter()
        .filter(|(_name, query)| match query.conn.0 {
            plan::Dialect::Mysql => false,
            plan::Dialect::Sqlite => true,
        })
        .map(move |(_name, query)| {
            let pool = sqlite_conns.get(&query.conn.1).map(|p| p.clone()).unwrap();
            let prog = query.read_sql().unwrap();
            warp::get()
                .and(warp::path(prefix_cloned.clone()))
                .and(warp::path(query.path))
                .and(
                    warp::query::raw()
                        .or(warp::any().map(|| String::new()))
                        .unify(),
                )
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
