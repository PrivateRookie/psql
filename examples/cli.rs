use std::process::exit;

use psql::parser::Program;
use sqlparser::dialect::MySqlDialect;
fn main() {
    let sql = "
--? age: num = 10 // useful help message
--? pattern: str // help
--? addrs: [str] = ['sh', 'beijing'] // address
--? pp: [num] // 必须使用???
select name from t where age=@age and name like @pattern and addr in @addrs and scores in @pp
";
    pretty_env_logger::init();
    let dialect = MySqlDialect {};
    let prog = Program::parse(&dialect, sql).unwrap();
    let mut opts = getopts::Options::new();
    prog.add_options(&mut opts);
    match prog.get_matches(&opts, &std::env::args().collect()) {
        Ok(values) => match prog.render(&dialect, &values) {
            Ok(stmts) => {
                println!(
                    "{:?}",
                    stmts
                        .iter()
                        .map(|stmt| stmt.to_string())
                        .collect::<String>()
                );
            }
            Err(e) => {
                println!("{}", e);
                exit(1);
            }
        },
        Err(e) => {
            println!("{}\n", e);
            println!("{}", opts.usage("PSQL"));
            exit(1);
        }
    }
}
