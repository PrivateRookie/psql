use std::collections::HashSet;

use sqlparser::tokenizer::Token;
use thiserror::Error;

use crate::parser::InnerTy;

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
    #[error("missing context value {0}")]
    MissingContextValue(String),
    #[error("{0}")]
    ParseError(sqlparser::parser::ParserError),
    #[error("param line parse error {0}")]
    ParamParseError(String),
    #[error("invalid arg value {0} for {1:?}")]
    InvalidArgValue(String, InnerTy),
    #[error("{0:?}")]
    TokenizeError(sqlparser::tokenizer::TokenizerError),
    #[error("expect end of statement, got {0:?}")]
    ExpectEndOfStatement(Token),
}
