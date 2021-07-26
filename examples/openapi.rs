use indexmap::IndexMap;
use openapiv3::{Contact, Info, OpenAPI, Operation, PathItem, ReferenceOr, Server};
use psql::parser::Program;
use sqlparser::dialect::MySqlDialect;

fn main() {
    let sql = "
    --? age: num = 10 // 一些有用的描述
    --? pattern: str // 参数说明
    --? addrs: [str] = ['sh', 'beijing'] // 默认值还没能整合进文档
    --? pp: [num] // 没有默认值, 则默认必须
    select name from t where age=@age and name like @pattern and addr in @addrs and scores in @pp
    ";
    pretty_env_logger::init();
    let dialect = MySqlDialect {};
    let prog = Program::tokenize(&dialect, sql).unwrap();
    let openapi = "3.0.3".to_string();
    let info = Info {
        title: "PSQL openapiv3".to_string(),
        description: Some("PSQL openapi demo".to_string()),
        terms_of_service: None,
        contact: Some(Contact {
            name: Some("PrivateRookie".to_string()),
            url: Some("https://github.com/PrivateRookie".to_string()),
            email: Some("xdsailfish@gmail.com".to_string()),
            extensions: Default::default(),
        }),
        license: None,
        version: "3.0.0".to_string(),
        extensions: Default::default(),
    };
    let server = Server {
        url: "api".to_string(),
        ..Default::default()
    };
    let servers = vec![server];
    let get_op = Operation {
        summary: Some("psql openapi demo".to_string()),
        parameters: prog.generate_openapi(),
        ..Default::default()
    };

    let mut paths = IndexMap::new();
    paths.insert(
        "/demo".to_string(),
        ReferenceOr::Item(PathItem {
            get: Some(get_op),
            ..Default::default()
        }),
    );

    let doc = OpenAPI {
        openapi,
        info,
        servers,
        paths,
        ..Default::default()
    };
    println!("{}", serde_json::to_string_pretty(&doc).unwrap());
}
