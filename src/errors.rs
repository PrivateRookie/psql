use std::collections::HashSet;

use sqlparser::tokenizer::Token;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PSqlError {
    #[error("invalid variable, expect identifier, found {0}")]
    InvalidVariable(Token),
    #[error("unused params {0:?}")]
    UnusedParams(HashSet<String>),
    #[error("missing params {0:?}")]
    MissingParams(HashSet<String>),
    #[error("duplicated param {0}")]
    DuplicatedParam(String),
}
