import "../types"
import { Axios } from "axios";
export * from ".types"

export const DBDialect = {
  mysql: "mysql",
  sqlite: "sqlite",
  unknown: "unknown"
}

/**
 *
 *
 * @param {string} conn
 * @param {string} op
 * @return {string}
 */
const notSupportSql = (conn, op) => {
  return `SELECT 'error' AS \`status\`, '${conn} do not support ${op} operation' AS \`msg\``;
}

/**
 *
 *
 * @param {DBDialect} dialect
 * @param {string} conn
 * @returns {NewQuery}
 */
export const schemaQuery = (dialect, conn) => {
  let sql = "";
  if (dialect == "mysql") {
    sql = "SELECT DATABASE() AS `db`"
  } else if (dialect == "sqlite") {
    sql = "SELECT '" + conn + "' AS `db`, 'sqlite do not support database() function!' as `msg`"
  } else {
    sql = "SELECT '" + conn + "' AS `db`, 'unknown database dialect' as `msg`"
  }
  return {
    name: "schema",
    sql,
    method: "GET",
    conn,
    summary: "get database name",
    tags: ["database_meta"],
    path: `${conn}/__meta/schema`
  };
}

/**
 *
 *
 * @param {DBDialect} dialect
 * @param {string} conn
 * @returns {NewQuery}
 */
export const tablesQuery = (dialect, conn) => {
  let sql = notSupportSql(conn, "list table");
  if (dialect == "mysql") {
    sql = "SELECT `table_name` AS `name`, `engine`\n" +
      "FROM information_schema.tables\n" +
      "WHERE `table_type` = 'BASE TABLE' AND `table_schema` = DATABASE()"
  } else if (dialect == "sqlite") {
    sql = "SELECT `tbl_name` AS `name`\n" +
      "FROM sqlite_master\n" +
      "WHERE type = 'table' AND `tbl_name` not like 'sqlite_%'"
  }
  return {
    name: "tables",
    conn,
    method: "GET",
    summary: "list database tables",
    sql,
    path: `${conn}/__meta/tables`,
    tags: ["database_meta"]
  }
}

/**
 *
 *
 * @param {DBDialect} dialect
 * @param {string} conn
 * @returns {NewQuery}
 */
export const tableIndex = (dialect, conn) => {
  let sql = notSupportSql(conn, "get table index");
  if (dialect == "mysql") {
    sql = `--? table: str // 表名
    select
        TABLE_SCHEMA AS \`db\`, TABLE_NAME AS \`table\`, NON_UNIQUE AS \`can_duplicate\`, INDEX_NAME AS \`name\`, COLUMN_NAME AS \`column_name\`, INDEX_TYPE AS \`type\`
    from information_schema.STATISTICS
    where table_name = @table AND TABLE_SCHEMA = DATABASE()
    `;
  } else if (dialect == "sqlite") {
    sql = `--? table: str // 表名
    SELECT \`name\`
    FROM sqlite_master
    WHERE type = 'index' AND tbl_name = @table
    `;
  }

  return {
    name: "table_index",
    conn,
    method: "GET",
    summary: "list table index",
    sql,
    path: `${conn}/__meta/table_index`,
    tags: ["database_meta"]
  }
}

/**
 *
 *
 * @param {DBDialect} dialect
 * @param {string} conn
 * @returns {NewQuery}
 */
export const tableColumn = (dialect, conn) => {
  let sql = notSupportSql(conn, "get table columns");
  if (dialect == "mysql") {
    sql = `--? table: str // 表名称
    select
        TABLE_SCHEMA AS \`db\`, COLUMN_NAME AS \`column_name\`, COLUMN_DEFAULT AS \`default_value\`, IS_NULLABLE AS \`is_nullable\`, DATA_TYPE AS \`type\`, COLUMN_KEY AS \`pk\`
    from information_schema.columns
    where table_name = @table AND \`TABLE_SCHEMA\` = DATABASE() `;
  } else if (dialect == "sqlite") {
    sql = `--? table: str // 表名称
    SELECT \`name\` AS \`column_name\`, \`dflt_value\` AS \`default_value\`, \`notnull\` AS \`is_nullable\`, \`type\`, \`pk\`
    FROM pragma_table_info(@table)`;
  }
  return {
    name: "table_column",
    conn,
    sql,
    method: "GET",
    summary: "list table column",
    path: `${conn}/__meta/table_column`,
    tags: ["database_meta"]
  }
}

/**
 *
 *
 * @param {DBDialect} dialect
 * @param {string} conn
 * @returns {NewQuery}
 */
export const tableFk = (dialect, conn) => {
  let sql = notSupportSql("get table foreign key");
  if (dialect == "mysql") {
    sql = `--? table: str // 表名称
    SELECT
        CONSTRAINT_SCHEMA AS \`db\`, CONSTRAINT_NAME AS \`name\`, UPDATE_RULE as \`update_rule\`, DELETE_RULE as \`delete_rule\`, TABLE_NAME as \`table\`, REFERENCED_TABLE_NAME as \`referenced_table\`
    FROM information_schema.REFERENTIAL_CONSTRAINTS
    WHERE \`db\` = DATABASE() AND \`TABLE_NAME\` = @table`;
  } else if (dialect == "sqlite") {
    sql = `--? table: str // 表名称
    SELECT \`from\` AS \`name\`, @table AS \`table\`, \`table\` AS \`referenced_table\`
    FROM pragma_foreign_key_list(@table)`;
  }
  return {
    name: "table_fk",
    conn,
    sql,
    method: "GET",
    summary: "get foreign key",
    path: `${conn}/__meta/table_fk`,
    tags: ["database_meta"]
  }
}

/**
 *
 *
 * @param {DBDialect} dialect
 * @param {string} conn
 * @returns {NewQuery}
 */
export const allFk = (dialect, conn) => {
  let sql = notSupportSql(conn, "get all foreign keys");
  if (dialect == "mysql") {
    sql = `select
    CONSTRAINT_SCHEMA AS \`db\`, CONSTRAINT_NAME AS \`name\`, UPDATE_RULE as \`update_rule\`, DELETE_RULE as \`delete_rule\`, TABLE_NAME as \`table\`, REFERENCED_TABLE_NAME as \`referenced_table\`
    from information_schema.REFERENTIAL_CONSTRAINTS
    WHERE \`CONSTRAINT_SCHEMA\` = DATABASE()`
  } else if (dialect == "sqlite") {
    sql = `SELECT
    p.\`from\`, m.name AS \`table\`, p."table" AS \`referenced_table\`
FROM
    sqlite_master m
    JOIN pragma_foreign_key_list(m.name) p ON m.name != p.\`table\`
WHERE m.type = 'table'
ORDER BY m.name`
  }
  return {
    name: "fk",
    conn,
    sql,
    method: "GET",
    summary: "list all foreign key of a database",
    path: `${conn}/__meta/fk`
  };
}

/**
 *
 * @param {Axios} axios
 * @param {string} conn
 * @param {DBDialect} dialect
 * @return {*}
 */
const postAddConn = (axios, conn, dialect) => {
  return addQuery(axios, [
    schemaQuery(dialect, conn),
    tablesQuery(dialect, conn),
    tableIndex(dialect, conn),
    tableColumn(dialect, conn),
    tableFk(dialect, conn),
    allFk(dialect, conn)
  ])
}

/**
* add database connection
* @param {Axios} axios
* @param {string} name connection name
* @param {string} uri database connection uri
*/
export async function addConn(axios, name, uri) {
  const { data } = axios.post("api/add_conn", [{ name, uri }]);
  const dialect = detectDialect(uri);
  await postAddConn(axios, name, dialect);
  return data;
}

/**
 * batch add database query
 * @param {Axios} axios
 * @param {Array<NewQuery>} queries
 */
export async function addQuery(axios, queries) {
  const { data } = await axios.post("api/add_query", queries);
  return data;
}

export async function testConnective(axios, uri) {
  const { data } = await axios.post("api/__util/test_connective", { uri });
  return data;
}
export async function dbTables(axios, db) {
  const { data } = await axios.get(`api/${db}/__meta/tables`);
  return data;
}
export async function tableColumns(axios, db, table) {
  const { data } = await axios.get(`api/${db}/__meta/table_column`, { params: { table } });
  return data;
}
export async function listTable(axios, db, table) {
  const { data } = await axios.get(`api/${db}/${table}/list`);
  return data;
}

export async function updateRow(axios, db, table, newRow) {
  const { data } = await axios.put(`api/${db}/${table}/update`, newRow);
  return data;
}

export async function deleteRow(axios, db, table, data) {
  const resp = await axios.delete(`api/${db}/${table}/delete`, { data });
  return resp.data;
}

export async function batchDeleteRow(axios, db, table, data) {
  const resp = await axios.delete(`api/${db}/${table}/batch_delete`, { data });
  return resp.data;
}

export async function createRow(axios, db, table, newRow) {
  const { data } = await axios.post(`api/${db}/${table}/create`, newRow);
  return data;
}
/**
 * show list table
 * @param {Axios} axios
 * @param {string} pattern table name pattern, support mysql style match
 * @param {string} db, database name
 * @return {Promise<Table[]>}
 */
export async function listTables(axios, pattern = null, db = "default") {
  const { data } = await axios.get("api/tables", { params: { pattern, db } });
  return data;
}
/**
 * get table indexes
 * @param {Axios} axios
 * @param {string} table table name
 * @return {Promise<Index[]>}
 */
export async function tableIndexes(axios, table) {
  const { data } = await axios.get("api/indexes", { params: { table } });
  return data;
}
/**
 * get all foreign key
 * @return {Promise<ForeignKey[]>}
 */
export async function listAllForeignKey(axios,) {
  const { data } = await axios.get("api/all_fk");
  return data;
}
/**
 * get table foreign key
 * @param {string} table table name
 * @return {Promise<ForeignKey[]>}
 */
export async function tableForeignKey(axios, table) {
  const { data } = await axios.get("api/table_fk", { params: { table } });
  return data;
}


/**
 * detect database dialect according to connection uri
 *
 * @param {string} uri
 * @returns {DBDialect}
 */
export const detectDialect = (uri) => {
  if (uri.startsWith("mysql")) {
    return "mysql"
  } else if (uri.startsWith("sqlite")) {
    return "sqlite"
  } else {
    return "unknown"
  }
}