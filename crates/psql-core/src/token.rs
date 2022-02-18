use sqlparser::tokenizer::Token;

#[derive(Debug, PartialEq, Clone)]
pub enum VariableToken {
    Var(String),
    Normal(Token),
}
