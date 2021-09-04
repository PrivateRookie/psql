use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

fn default_groups() -> Vec<String> {
    vec!["default".to_string()]
}

/// ER 模型
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ERModel {
    /// entities 组定义, 默认有一个 default 组
    #[serde(default = "default_groups")]
    pub groups: Vec<String>,
    /// entity 定义
    pub entities: Vec<Entity>,
    /// entity 关系描述
    pub relationships: Vec<Relationship>,
}

/// Entity 描述
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Entity {
    /// 名称
    pub name: String,
    /// 描述
    #[serde(default)]
    pub desc: String,
    /// entity 所具有的列
    pub columns: Vec<Column>,
}

/// Entity 列描述
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Column {
    /// 列名
    pub name: String,
    /// 列类型
    pub ty: DataType,
}

/// Column 类型
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum DataType {
    /// 文本
    #[serde(rename = "text")]
    Text,
    /// 整数
    #[serde(rename = "int")]
    Int,
    /// 小数
    #[serde(rename = "float")]
    Float,
    /// 主键
    #[serde(rename = "pk")]
    PrimaryKey,
}

/// 关系描述
///
/// **left** has one | many **right** ==> right 上有 left 的外键
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Relationship {
    /// 外键中被引用的 entity
    pub left: Entity,
    /// 具有外键的 entity
    pub right: Entity,
    /// 关系 one | many
    pub ty: RelationType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum RelationType {
    #[serde(rename = "one")]
    One,
    #[serde(rename = "many")]
    Many,
}

fn main() {
    let schema = schemars::schema_for!(ERModel);
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}
