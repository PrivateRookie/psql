use std::{fs::File, io::Read, path::PathBuf, process::exit};

use psql::http::{run_dynamic_http, Plan};
use schemars::schema_for;
use structopt::StructOpt;

/// PSQL http service demo
#[derive(Clone, StructOpt)]
struct Args {
    /// plan.toml file path
    #[structopt(short, long, default_value = "plan.toml")]
    plan: PathBuf,
    /// print plan.toml json schema and exit
    #[structopt(short, long)]
    show_schema: bool,
    /// print generated openapi json and exit
    #[structopt(short = "o", long = "show_doc")]
    show_openapi_doc: bool,
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    pretty_env_logger::init();
    let args = Args::from_args();
    if args.show_schema {
        let schema = schema_for!(Plan);
        println!("{}", serde_json::to_string_pretty(&schema).unwrap());
        std::process::exit(0);
    }
    match File::open(&args.plan) {
        Ok(mut file) => {
            let mut content = String::new();
            match file.read_to_string(&mut content) {
                Ok(_) => match toml::from_str::<Plan>(&content) {
                    Ok(plan) => {
                        let doc = plan.openapi_doc();
                        if args.show_openapi_doc {
                            println!("{}", serde_json::to_string_pretty(&doc).unwrap());
                            std::process::exit(0);
                        }
                        match plan.create_connections().await {
                            Ok((mysql_conns, sqlite_conns)) => {
                                run_dynamic_http(plan, mysql_conns, sqlite_conns).await
                            }
                            Err(e) => {
                                println!("{}", e);
                                std::process::exit(1);
                            }
                        }
                    }
                    Err(e) => {
                        println!("invalid config file {:#?}", e);
                        exit(1);
                    }
                },
                Err(e) => {
                    println!("{:#?}", e);
                    exit(1);
                }
            }
        }
        Err(e) => {
            println!("{:#}", e);
            exit(1);
        }
    }
}
