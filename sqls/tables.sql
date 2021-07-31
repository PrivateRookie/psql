--? pattern: str = '%%'// 表名字
--? db: str = 'default' // 数据库名
select
    table_schema as db, table_name, engine
from tables
where table_name like @pattern and table_schema like @db