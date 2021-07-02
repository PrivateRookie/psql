use pest::Parser;

#[derive(pest_derive::Parser)]
#[grammar = "test.pest"]
// #[grammar = "grammer.pest"]
pub struct PSQLParser;

#[derive(Debug, Clone)]
pub enum InnerTy {
    Str,
    Int,
    Float,
    Raw,
}

#[derive(Debug, Clone)]
pub enum InnerVal {
    String(String),
    Int(i64),
    Float(f64),
    Raw(String),
}


#[derive(Debug, Clone)]
pub enum Token {
    HelpMsg(String),
    Ident(String),
    Ty(InnerTy),
    Val(InnerVal),
}
