use crate::parser::{ParamValue, Program};
use openapiv3::OpenAPI;
use output::{QueryOutput, QueryOutputMapSer};
pub use plan::Plan;
use querystring::querify;
use serde::{Deserialize, Serialize};
use sqlparser::dialect::MySqlDialect;
use std::{collections::HashMap, convert::Infallible};
use warp::Filter;

mod index;
pub mod output;
pub mod plan;

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
    async fn serve_doc(doc: OpenAPI) -> Result<impl warp::Reply, Infallible> {
        Ok(warp::reply::json(&doc))
    }
    async fn serve_query(
        qs: String,
        prog: Program,
        pool: sqlx::MySqlPool,
    ) -> Result<impl warp::Reply, Infallible> {
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
        let rendered: String = prog
            .render(&MySqlDialect {}, &context)
            .unwrap()
            .iter()
            .map(|stmt| stmt.to_string())
            .collect();
        let code = warp::http::StatusCode::OK;
        let result = sqlx::query(&rendered)
            .fetch_all(&pool)
            .await
            .map(|rows| QueryOutput { rows })
            .unwrap();
        Ok(warp::reply::with_status(
            warp::reply::json(&QueryOutputMapSer(&result)),
            code,
        ))
    }
    let prefix = plan.prefix.clone();
    let doc_path = plan.doc_path.clone();
    let doc_route = warp::get()
        .and(warp::path(prefix.clone()))
        .and(warp::path(plan.doc_path.clone()))
        .and(warp::any().map(move || doc.clone()))
        .and_then(serve_doc);
    let index = warp::get()
        .and(warp::path("index"))
        .and(warp::any().map(move || format!("{}/{}", prefix, doc_path)))
        .and_then(index::serve_index);
    let queries = plan.queries.clone();
    let prefix = plan.prefix.clone();
    let api_routes = queries
        .into_iter()
        .map(move |(_name, query)| {
            let pool = conns.get(&query.conn).map(|p| p.clone()).unwrap();
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
