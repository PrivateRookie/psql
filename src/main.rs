use psql::parser::Program;
use sqlparser::dialect::MySqlDialect;
use sqlparser::parser::Parser;
fn main() {
    let sql = "
--? age: num = 10 // useful help message
--? pattern: str // help
--? addrs: [str] = ['sh', 'beijing'] // address
--? pp: [num] // 必须使用???
-- single line
select * from table where age=@age where name like @pattern and addr in @addrs and scores in @pp;
";
    let dialect = MySqlDialect {};
    let prog = Program::tokenize(&dialect, sql).unwrap();
    let values = prog.get_matches();
    dbg!(prog.render(&values));
}
