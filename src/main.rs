use psql::parser::Program;
use sqlparser::dialect::MySqlDialect;
use sqlparser::parser::Parser;
fn main() {
    let sql = "
--? age: num = 10 // useful help message
--? pattern: str // :+))))))
--? invalid
-- single line
select * from table where age=@age where name like @pattern
";
    let dialect = MySqlDialect {};
    let prog = Program::tokenize(&dialect, sql).unwrap();
    prog.get_matches();
}
