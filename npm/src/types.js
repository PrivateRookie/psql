/**
   * @typedef { Object } NewQuery
   * @property {string} name - query name
   * @property {string} conn - conn that this query belongs to
   * @property {"GET" | "POST" | "PUT" | "PATCH" | "DELETE"} method - http method of this query
   * @property {string | null} summary - api short summary
   * @property {string} sql - query sql content, support paras
   * @property {string} path - api relative url path
   * @property {Array<string>} tags - api tags
*/


/**
 * @typedef { "mysql" | "sqlite" | "unknown" } DBDialect
  */
