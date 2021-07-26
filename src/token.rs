use sqlparser::tokenizer::Token;

#[derive(Debug, PartialEq)]
pub enum VariableToken {
    Var(String),
    Normal(Token),
}
