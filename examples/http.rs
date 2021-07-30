use psql::http::{run_http, Plan};
use schemars::schema_for;

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
    match plan.create_connections().await {
        Ok(conns) => run_http(plan, doc, conns).await,
        Err(e) => {
            println!("{}", e);
            std::process::exit(1);
        }
    }
}
