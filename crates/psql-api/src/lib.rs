use psql::http::{
    plan::{Method, Query},
    NewQuery,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DBDialect {
    #[serde(rename = "mysql")]
    Mysql,
    #[serde(rename = "sqlite")]
    Sqlite,
    #[serde(rename = "unknown")]
    Unknown,
}

impl DBDialect {
    pub fn detect(uri: &str) -> Self {
        if uri.starts_with("mysql") {
            Self::Mysql
        } else if uri.starts_with("sqlite") {
            Self::Sqlite
        } else {
            Self::Unknown
        }
    }
}

type Resp = reqwest::Result<reqwest::Response>;

fn not_support_sql(name: &str, op: &str) -> String {
    format!("SELECT 'error' AS `status`, '{name} do not support {op} operation' AS `msg`")
}

fn meta_tags() -> Vec<String> {
    vec!["database_meta".to_string()]
}

/// get current database name query params
pub fn schema_query(dialect: &DBDialect, conn: &str) -> NewQuery {
    let sql = match dialect {
        DBDialect::Mysql => "SELECT DATABASE() AS `db`".to_string(),
        DBDialect::Sqlite => format!(
            "SELECT '{conn}' AS `db`, 'sqlite do not support database() function!' as `msg`"
        ),
        DBDialect::Unknown => {
            format!("SELECT '{conn}' AS `db`, 'unknown database dialect' as `msg`")
        }
    };
    NewQuery {
        name: "schema".to_string(),
        query: Query {
            conn: conn.to_string(),
            method: Method::Get,
            summary: Some("get database name".to_string()),
            sql,
            path: format!("{conn}/__meta/schema"),
            tags: meta_tags(),
        },
    }
}

/// list database all table query params
pub fn tables_query(dialect: &DBDialect, conn: &str) -> NewQuery {
    let sql = match dialect {
        DBDialect::Mysql => format!(
            r#"SELECT `table_name` AS `name`, `engine`
        FROM information_schema.tables"
        WHERE `table_type` = 'BASE TABLE' AND `table_schema` = DATABASE()"#
        ),
        DBDialect::Sqlite => format!(
            r#"SELECT `tbl_name` AS `name`
        FROM sqlite_master
        WHERE type = 'table' AND `tbl_name` not like 'sqlite_%'"#
        ),
        DBDialect::Unknown => not_support_sql(conn, "list table"),
    };
    NewQuery {
        name: "tables".to_string(),
        query: Query {
            conn: conn.into(),
            method: Method::Get,
            summary: None,
            sql,
            path: format!("{conn}/__meta/tables"),
            tags: meta_tags(),
        },
    }
}

/// get table indexes query params
pub fn table_index_query(dialect: &DBDialect, conn: &str) -> NewQuery {
    let sql = match dialect {
        DBDialect::Mysql => format!(
            r#"--? table: str // 表名
        select
            TABLE_SCHEMA AS `db`, TABLE_NAME AS `table`, NON_UNIQUE AS `can_duplicate`, INDEX_NAME AS `name`, COLUMN_NAME AS `column_name`, INDEX_TYPE AS `type`
        from information_schema.STATISTICS
        where table_name = @table AND TABLE_SCHEMA = DATABASE()"#
        ),
        DBDialect::Sqlite => format!(
            r#"--? table: str // 表名
        SELECT `name`
        FROM sqlite_master
        WHERE type = 'index' AND tbl_name = @table"#
        ),
        DBDialect::Unknown => not_support_sql(conn, "get table index"),
    };
    NewQuery {
        name: "table_index".to_string(),
        query: Query {
            conn: conn.to_string(),
            method: Method::Get,
            summary: None,
            sql,
            path: format!("{conn}/__meta/table_index"),
            tags: meta_tags(),
        },
    }
}

/// list table columns query params
pub fn table_column_query(dialect: &DBDialect, conn: &str) -> NewQuery {
    let sql = match dialect {
        DBDialect::Mysql => format!(
            r#"--? table: str // 表名称
        select
            TABLE_SCHEMA AS \`db\`, COLUMN_NAME AS \`column_name\`, COLUMN_DEFAULT AS \`default_value\`, IS_NULLABLE AS \`is_nullable\`, DATA_TYPE AS \`type\`, COLUMN_KEY AS \`pk\`
        from information_schema.columns
        where table_name = @table AND \`TABLE_SCHEMA\` = DATABASE() "#
        ),
        DBDialect::Sqlite => format!(
            r#"--? table: str // 表名称
        SELECT \`name\` AS \`column_name\`, \`dflt_value\` AS \`default_value\`, \`notnull\` AS \`is_nullable\`, \`type\`, \`pk\`
        FROM pragma_table_info(@table)"#
        ),
        DBDialect::Unknown => not_support_sql(conn, "get table columns"),
    };
    NewQuery {
        name: "table_column".to_string(),
        query: Query {
            conn: conn.to_string(),
            method: Method::Get,
            summary: None,
            sql,
            path: format!("{conn}/__meta/table_column"),
            tags: meta_tags(),
        },
    }
}

pub fn table_fk_query(dialect: &DBDialect, conn: &str) -> NewQuery {
    let sql = match dialect {
        DBDialect::Mysql => format!(
            r#"--? table: str // 表名称
        SELECT
            CONSTRAINT_SCHEMA AS \`db\`, CONSTRAINT_NAME AS \`name\`, UPDATE_RULE as \`update_rule\`, DELETE_RULE as \`delete_rule\`, TABLE_NAME as \`table\`, REFERENCED_TABLE_NAME as \`referenced_table\`
        FROM information_schema.REFERENTIAL_CONSTRAINTS
        WHERE \`db\` = DATABASE() AND \`TABLE_NAME\` = @table"#
        ),
        DBDialect::Sqlite => format!(
            r#"--? table: str // 表名称
        SELECT \`from\` AS \`name\`, @table AS \`table\`, \`table\` AS \`referenced_table\`
        FROM pragma_foreign_key_list(@table)"#
        ),
        DBDialect::Unknown => not_support_sql(conn, "get table foreign key"),
    };
    NewQuery {
        name: "table_fk".to_string(),
        query: Query {
            conn: conn.to_string(),
            method: Method::Get,
            summary: None,
            sql,
            path: format!("{conn}/__meta/table_fk"),
            tags: meta_tags(),
        },
    }
}

pub fn all_fk_query(dialect: &DBDialect, conn: &str) -> NewQuery {
    let sql = match dialect {
        DBDialect::Mysql => format!(
            r#"select
        CONSTRAINT_SCHEMA AS \`db\`, CONSTRAINT_NAME AS \`name\`, UPDATE_RULE as \`update_rule\`, DELETE_RULE as \`delete_rule\`, TABLE_NAME as \`table\`, REFERENCED_TABLE_NAME as \`referenced_table\`
        from information_schema.REFERENTIAL_CONSTRAINTS
        WHERE \`CONSTRAINT_SCHEMA\` = DATABASE()"#
        ),
        DBDialect::Sqlite => format!(
            r#"SELECT
        p.\`from\`, m.name AS \`table\`, p."table" AS \`referenced_table\`
    FROM
        sqlite_master m
        JOIN pragma_foreign_key_list(m.name) p ON m.name != p.\`table\`
    WHERE m.type = 'table'
    ORDER BY m.name"#
        ),
        DBDialect::Unknown => not_support_sql(conn, "get all foreign keys"),
    };
    NewQuery {
        name: "fk".to_string(),
        query: Query {
            conn: conn.to_string(),
            method: Method::Get,
            summary: None,
            sql,
            path: format!("{conn}/__meta/fk"),
            tags: meta_tags(),
        },
    }
}

/// add new query
pub async fn add_query(client: &Client, base_url: &str, queries: Vec<NewQuery>) -> Resp {
    client
        .post(format!("{base_url}/api_add_query"))
        .json(&queries)
        .send()
        .await
}

/// add database connection
pub async fn add_conn(client: &Client, base_url: &str, name: &str, db_uri: &str) -> Resp {
    let resp = client
        .post(format!("{base_url}/api/add_conn"))
        .json(&json!({
            "name": name,
            "uri": db_uri
        }))
        .send()
        .await?;
    let dialect = DBDialect::detect(db_uri);
    add_query(
        client,
        base_url,
        vec![
            schema_query(&dialect, name),
            tables_query(&dialect, name),
            table_index_query(&dialect, name),
            table_column_query(&dialect, name),
            table_fk_query(&dialect, name),
            all_fk_query(&dialect, name),
        ],
    )
    .await?;
    Ok(resp)
}

/// test database connective
pub async fn test_connective(client: &Client, base_url: &str, db_uri: &str) -> Resp {
    client
        .post(format!("{base_url}/api/__util/test_connective"))
        .json(&json!({ "uri": db_uri }))
        .send()
        .await
}

/// list db tables
pub async fn db_tables(client: &Client, base_url: &str, db: &str) -> Resp {
    client
        .get(format!("{base_url}/api/{db}/__meta/tables"))
        .send()
        .await
}

/// list table columns
pub async fn table_columns(client: &Client, base_url: &str, db: &str, table: &str) -> Resp {
    client
        .get(format!("{base_url}/api/{db}/__meta/table_column"))
        .json(&json!({
            "params": {
                "table": table
            }
        }))
        .send()
        .await
}

/// list table indexes
pub async fn table_indexes(client: &Client, base_url: &str, db: &str, table: &str) -> Resp {
    client
        .get(format!("{base_url}/api/{db}/__meta/table_index"))
        .json(&json!({
            "params": {
                "table": table
            }
        }))
        .send()
        .await
}

/// list table foreign keys
pub async fn table_fk(client: &Client, base_url: &str, db: &str, table: &str) -> Resp {
    client
        .get(format!("{base_url}/api/{db}/__meta/table_fk"))
        .json(&json!({
            "params": {
                "table": table
            }
        }))
        .send()
        .await
}
