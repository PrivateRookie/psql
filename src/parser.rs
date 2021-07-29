use crate::{errors::PSqlError, token::VariableToken};
use nom::{
    branch::alt,
    bytes::complete::{is_not, tag, take_while},
    character::complete::{alpha1, alphanumeric1, char},
    combinator::{map, opt, recognize},
    error::context,
    error::{ContextError as NomContextError, ParseError as NomParseError},
    multi::{many0, separated_list0},
    number::complete::double as nom_double,
    sequence::{pair, preceded, terminated, tuple},
    IResult,
};
use openapiv3::{
    ArrayType, NumberType, Parameter, ParameterData, ParameterSchemaOrContent, ReferenceOr, Schema,
    SchemaData, SchemaKind, StringType, Type,
};
use sqlparser::{
    dialect::Dialect,
    tokenizer::{Token, Whitespace},
};
use std::{
    collections::{HashMap, HashSet},
    process::exit,
};

#[derive(Debug, PartialEq, Clone)]
pub enum ParamValue {
    Str(String),
    Num(f64),
    Raw(String),
    Array(Vec<ParamValue>),
}

impl ToString for ParamValue {
    fn to_string(&self) -> String {
        match self {
            ParamValue::Str(str) => format!("'{}'", str),
            ParamValue::Num(num) => num.to_string(),
            ParamValue::Raw(raw) => raw.clone(),
            ParamValue::Array(arr) => {
                format!(
                    "({})",
                    arr.iter()
                        .map(|i| i.to_string())
                        .collect::<Vec<String>>()
                        .join(", ")
                )
            }
        }
    }
}

impl ParamValue {
    pub fn into_token<D: Dialect>(self, dialect: &D) -> Vec<Token> {
        match self {
            ParamValue::Str(val) => vec![Token::SingleQuotedString(val)],
            ParamValue::Num(val) => vec![Token::Number(val.to_string(), false)],
            ParamValue::Raw(val) => sqlparser::tokenizer::Tokenizer::new(dialect, &val)
                .tokenize()
                .unwrap(),
            ParamValue::Array(val) => {
                let mut tokens = vec![Token::LParen];
                let length = val.len();
                for (idx, item) in val.into_iter().enumerate() {
                    tokens.extend(item.into_token(dialect));
                    if idx + 1 != length {
                        tokens.push(Token::Comma);
                    }
                }
                tokens.push(Token::RParen);
                tokens
            }
        }
    }
    /// parse from arg string
    ///
    /// **NOTE** string parsed from arg isn't wrapped with `'` or `"`
    pub fn from_arg_str(ty: &InnerTy, arg_str: &str) -> Result<Self, PSqlError> {
        match ty {
            InnerTy::Str => Ok(ParamValue::Str(arg_str.to_string())),
            InnerTy::Num => {
                let (remain, val) = double::<nom::error::VerboseError<&str>>(arg_str)
                    .map_err(|e| PSqlError::ParamParseError(e.to_string()))?;
                if remain.is_empty() {
                    Ok(val)
                } else {
                    Err(PSqlError::InvalidArgValue(arg_str.to_string(), ty.clone()))
                }
            }
            InnerTy::Raw => {
                let (remain, val) = raw::<nom::error::VerboseError<&str>>(arg_str)
                    .map_err(|e| PSqlError::ParamParseError(e.to_string()))?;
                if remain.is_empty() {
                    Ok(val)
                } else {
                    Err(PSqlError::InvalidArgValue(arg_str.to_string(), ty.clone()))
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InnerTy {
    Str,
    Num,
    Raw,
}

impl ToString for InnerTy {
    fn to_string(&self) -> String {
        match self {
            InnerTy::Str => "str".to_string(),
            InnerTy::Num => "num".to_string(),
            InnerTy::Raw => "raw".to_string(),
        }
    }
}

impl InnerTy {
    fn to_openapi_schema_kind(&self) -> SchemaKind {
        match self {
            InnerTy::Str => SchemaKind::Type(Type::String(StringType::default())),
            InnerTy::Num => SchemaKind::Type(Type::Number(NumberType::default())),
            InnerTy::Raw => SchemaKind::Type(Type::String(StringType {
                pattern: Some("^#.*#$".to_string()),
                ..Default::default()
            })),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ParamTy {
    Basic(InnerTy),
    Array(InnerTy),
}

impl ToString for ParamTy {
    fn to_string(&self) -> String {
        match self {
            ParamTy::Basic(ty) => ty.to_string(),
            ParamTy::Array(ty) => format!("[{}]", ty.to_string()),
        }
    }
}

/// PSQL parameter
#[derive(Debug, PartialEq, Clone)]
pub struct Param {
    pub name: String,
    pub ty: ParamTy,
    pub default: Option<ParamValue>,
    pub help: String,
}

impl Param {
    pub fn to_openapi_param(&self) -> Parameter {
        let schema_kind = match &self.ty {
            ParamTy::Basic(inner_ty) => inner_ty.to_openapi_schema_kind(),
            ParamTy::Array(inner_ty) => SchemaKind::Type(Type::Array(ArrayType {
                items: ReferenceOr::Item(Box::new(Schema {
                    schema_kind: inner_ty.to_openapi_schema_kind(),
                    schema_data: Default::default(),
                })),
                min_items: None,
                max_items: None,
                unique_items: false,
            })),
        };
        Parameter::Query {
            parameter_data: ParameterData {
                name: self.name.clone(),
                description: Some(self.help.clone()),
                required: self.default.is_none(),
                deprecated: None,
                format: ParameterSchemaOrContent::Schema(ReferenceOr::Item(Schema {
                    schema_data: SchemaData::default(),
                    schema_kind,
                })),
                example: None,
                examples: Default::default(),
                explode: None,
                extensions: Default::default(),
            },
            allow_reserved: false,
            style: Default::default(),
            allow_empty_value: None,
        }
    }
}

fn double_quote_str<'a, E: NomParseError<&'a str> + NomContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, &'a str, E> {
    let not_quote_slash = is_not("\"\\");
    context(
        "double quote str",
        map(
            tuple((tag("\""), not_quote_slash, tag("\""))),
            |(_, str, _)| str,
        ),
    )(input)
}

fn single_quote_str<'a, E: NomParseError<&'a str> + NomContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, &'a str, E> {
    let not_quote_slash = is_not("'\\");
    context(
        "single quote str",
        map(
            tuple((tag("'"), not_quote_slash, tag("'"))),
            |(_, str, _)| str,
        ),
    )(input)
}

fn str<'a, E: NomParseError<&'a str> + NomContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, ParamValue, E> {
    context(
        "str",
        map(alt((single_quote_str, double_quote_str)), |val: &str| {
            ParamValue::Str(val.to_string())
        }),
    )(input)
}

fn double<'a, E: NomParseError<&'a str> + NomContextError<&'a str>>(
    input: &'a str,
) -> IResult<&str, ParamValue, E> {
    context("double", map(nom_double, ParamValue::Num))(input)
}

fn raw<'a, E: NomParseError<&'a str> + NomContextError<&'a str>>(
    input: &'a str,
) -> IResult<&str, ParamValue, E> {
    let not_quote_slash = is_not("#\\");
    context(
        "raw val",
        map(
            tuple((tag("#"), not_quote_slash, tag("#"))),
            |(_, str, _): (&str, &str, &str)| ParamValue::Raw(str.to_string()),
        ),
    )(input)
}

fn no_newline_sp<'a, E: NomParseError<&'a str> + NomContextError<&'a str>>(
    input: &'a str,
) -> IResult<&str, &str, E> {
    let chars = " \t";
    take_while(move |c| chars.contains(c))(input)
}

fn parse_array<
    'a,
    E: NomParseError<&'a str> + NomContextError<&'a str>,
    F: nom::Parser<&'a str, ParamValue, E>,
>(
    input: &'a str,
    f: F,
) -> IResult<&str, ParamValue, E> {
    context(
        "array",
        map(
            preceded(
                tuple((tag("["), no_newline_sp)),
                terminated(
                    separated_list0(tuple((no_newline_sp, tag(","), no_newline_sp)), f),
                    tuple((no_newline_sp, tag("]"))),
                ),
            ),
            ParamValue::Array,
        ),
    )(input)
}

fn identifier<'a, E: NomParseError<&'a str> + NomContextError<&'a str>>(
    input: &'a str,
) -> IResult<&str, String, E> {
    context(
        "identifier",
        map(
            recognize(pair(
                alt((alpha1, tag("_"))),
                many0(alt((alphanumeric1, tag("_")))),
            )),
            |val: &str| val.to_string(),
        ),
    )(input)
}

fn basic_ty<'a, E: NomParseError<&'a str> + NomContextError<&'a str>>(
    input: &'a str,
) -> IResult<&str, InnerTy, E> {
    context(
        "basic ty",
        alt((
            map(tag("str"), |_| InnerTy::Str),
            map(tag("num"), |_| InnerTy::Num),
            map(tag("raw"), |_| InnerTy::Raw),
        )),
    )(input)
}

fn parse_ty<'a, E: NomParseError<&'a str> + NomContextError<&'a str>>(
    input: &'a str,
) -> IResult<&str, ParamTy, E> {
    alt((
        context(
            "array ty",
            preceded(
                char('['),
                terminated(
                    map(
                        tuple((no_newline_sp, basic_ty, no_newline_sp)),
                        |(_, ty, _)| ParamTy::Array(ty),
                    ),
                    char(']'),
                ),
            ),
        ),
        map(basic_ty, ParamTy::Basic),
    ))(input)
}

fn parse_default<'a, E: NomParseError<&'a str> + NomContextError<&'a str>>(
    input: &'a str,
    ty: &ParamTy,
) -> IResult<&'a str, ParamValue, E> {
    match &ty {
        ParamTy::Basic(inner_ty) => match inner_ty {
            InnerTy::Str => str(input),
            InnerTy::Num => double(input),
            InnerTy::Raw => raw(input),
        },
        ParamTy::Array(inner_ty) => match inner_ty {
            InnerTy::Str => parse_array(input, str),
            InnerTy::Num => parse_array(input, double),
            InnerTy::Raw => parse_array(input, raw),
        },
    }
}

fn take_eq<'a, E: NomParseError<&'a str> + NomContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, (), E> {
    map(
        recognize(tuple((no_newline_sp, tag("="), no_newline_sp))),
        |_| (),
    )(input)
}

/// parse param line
fn param<'a, E: NomParseError<&'a str> + NomContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, Param, E> {
    let (input, (name, ty)) = map(
        tuple((
            tag("?"),
            no_newline_sp,
            identifier,
            no_newline_sp,
            tag(":"),
            no_newline_sp,
            parse_ty,
        )),
        |(_, _, name, _, _, _, ty)| (name, ty),
    )(input)?;
    let (input, default) = match take_eq::<nom::error::VerboseError<&str>>(input) {
        Ok((input, _)) => {
            let (input, default) = parse_default(input, &ty)?;
            (input, Some(default))
        }
        Err(_) => (input, None),
    };
    let (input, help) = context(
        "help",
        opt(map(
            tuple((no_newline_sp, tag("//"), no_newline_sp, is_not("\r\n"))),
            |(_, _, _, help)| help.to_string(),
        )),
    )(input)?;
    let param = Param {
        name,
        ty,
        default,
        help: help.unwrap_or_default(),
    };
    Ok((input, param))
}

#[test]
fn parse_param() {
    let cases = vec![
        ("complete num", "? age : num = 10 // help msg"),
        (
            "complete double quote str",
            "? addr: str = \"SH\"//where are you from?",
        ),
        (
            "complete single quote str",
            "? addr: str = 'SH'//where are you from?",
        ),
        (
            "complete raw",
            "? where: raw = #select * from ()# // insert raw",
        ),
        (
            "complete array",
            "? arr: [num] = [ 1, 2, 3 ] // array param",
        ),
        ("no default", "? age: num // help msg"),
        ("no help msg", "? age: num = 10"),
        ("simple", "? age: num"),
        ("invalid num", "? age: num = gx"),
        ("invalid num", "? age: num = "),
    ];
    for (name, input) in cases.iter() {
        println!(
            "[{}] {} -> {:?}",
            name,
            input,
            param::<nom::error::VerboseError<&str>>(input)
        );
    }
}

/// a sql file, may contains multi statements
#[derive(Debug, Clone)]
pub struct Program {
    pub params: Vec<Param>,
    pub tokens: Vec<VariableToken>,
}

impl Program {
    pub fn parse(dialect: &impl Dialect, program: &str) -> Result<Program, PSqlError> {
        let tokens = sqlparser::tokenizer::Tokenizer::new(dialect, program)
            .tokenize()
            .map_err(PSqlError::TokenizeError)?;
        let mut processed = vec![];
        let mut params = vec![];
        let mut expect_word = false;
        for token in tokens.into_iter() {
            match token {
                Token::AtSign => {
                    if expect_word {
                        return Err(PSqlError::InvalidVariable(token));
                    } else {
                        expect_word = true
                    }
                }
                Token::Word(word) => {
                    if expect_word {
                        processed.push(VariableToken::Var(word.to_string()));
                        expect_word = false
                    } else {
                        processed.push(VariableToken::Normal(Token::Word(word)))
                    }
                }
                Token::Whitespace(ws) => match ws {
                    Whitespace::SingleLineComment { comment, prefix } => {
                        if comment.starts_with('?') {
                            let (_, param) = param::<nom::error::VerboseError<&str>>(&comment)
                                .map_err(|e| PSqlError::ParamParseError(format!("{:#?}", e)))?;
                            params.push(param);
                        } else {
                            processed.push(VariableToken::Normal(Token::Whitespace(
                                Whitespace::SingleLineComment { comment, prefix },
                            )))
                        }
                    }
                    _ => processed.push(VariableToken::Normal(Token::Whitespace(ws))),
                },
                _ => {
                    if expect_word {
                        return Err(PSqlError::InvalidVariable(token));
                    } else {
                        processed.push(VariableToken::Normal(token))
                    }
                }
            }
        }
        // validation check
        let param_names_vec = params.iter().map(|p| p.name.clone());
        // 1. check duplication
        let mut param_names = HashSet::new();
        for p in param_names_vec.into_iter() {
            if !param_names.insert(p.clone()) {
                return Err(PSqlError::DuplicatedParam(p));
            }
        }
        let mut var_names = HashSet::new();
        for t in processed.iter() {
            if let VariableToken::Var(name) = t {
                var_names.insert(name.clone());
            }
        }
        // 2. check missing arguments
        let missing: HashSet<String> = var_names.difference(&param_names).cloned().collect();
        if !missing.is_empty() {
            return Err(PSqlError::MissingParams(missing));
        }
        // 3. check used arguments
        let unused: HashSet<String> = param_names.difference(&var_names).cloned().collect();
        if !unused.is_empty() {
            return Err(PSqlError::UnusedParams(unused));
        }
        Ok(Program {
            tokens: processed,
            params,
        })
    }

    /// generate command line options
    pub fn generate_options(&self) -> getopts::Options {
        let mut opts = getopts::Options::new();
        opts.optflag("h", "help", "print usage message");
        for p in self.params.iter() {
            match (&p.default, &p.ty) {
                (None, ParamTy::Basic(_)) => {
                    opts.reqopt(
                        "",
                        &p.name,
                        &p.help,
                        &format!("*<{}> {}", p.name.to_uppercase(), p.ty.to_string()),
                    );
                }
                (None, ParamTy::Array(_)) => {
                    opts.optmulti(
                        "",
                        &p.name,
                        &p.name,
                        &format!("*<{}> {}", p.name.to_uppercase(), p.ty.to_string()),
                    );
                }
                (Some(default), ParamTy::Basic(_)) => {
                    opts.optopt(
                        "",
                        &p.name,
                        &p.help,
                        &format!(
                            "[{}] {} {}",
                            p.name.to_uppercase(),
                            p.ty.to_string(),
                            default.to_string()
                        ),
                    );
                }
                (Some(default), ParamTy::Array(_)) => {
                    opts.optmulti(
                        "",
                        &p.name,
                        &p.help,
                        &format!(
                            "<{}> {} {}",
                            p.name.to_uppercase(),
                            p.ty.to_string(),
                            default.to_string()
                        ),
                    );
                }
            }
        }
        opts
    }

    /// generate open api doc parameters
    pub fn generate_openapi(&self) -> Vec<ReferenceOr<Parameter>> {
        self.params
            .iter()
            .map(|p| ReferenceOr::Item(p.to_openapi_param()))
            .collect()
    }

    /// read from args
    pub fn get_matches(
        &self,
        opts: &getopts::Options,
    ) -> Result<HashMap<String, ParamValue>, getopts::Fail> {
        use std::env::args;
        let cmd_args: Vec<String> = args()
            .collect::<Vec<String>>()
            .into_iter()
            .skip(1)
            .collect();
        if cmd_args.contains(&"-h".to_string()) || cmd_args.contains(&"--help".to_string()) {
            println!("{}", opts.usage("psql"));
            exit(0)
        }
        match opts.parse(&cmd_args) {
            Ok(matches) => {
                let mut values = HashMap::new();
                for p in self.params.iter() {
                    match &p.ty {
                        ParamTy::Basic(ty) => {
                            let ocr: Option<String> = matches.opt_str(&p.name);
                            match (ocr, p.default.clone()) {
                                (None, None) => {
                                    return Err(getopts::Fail::OptionMissing(p.name.clone()));
                                }
                                (None, Some(default)) => {
                                    values.insert(p.name.clone(), default);
                                }
                                (Some(arg_str), _) => {
                                    match ParamValue::from_arg_str(ty, &arg_str) {
                                        Ok(val) => {
                                            values.insert(p.name.clone(), val);
                                        }
                                        Err(e) => {
                                            return Err(getopts::Fail::UnexpectedArgument(
                                                format!("{}, {}", p.name, e),
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                        ParamTy::Array(ty) => {
                            let ocrs = matches.opt_strs(&p.name);
                            match (ocrs.is_empty(), p.default.clone()) {
                                (true, None) => {
                                    return Err(getopts::Fail::OptionMissing(p.name.clone()));
                                }
                                (true, Some(default)) => {
                                    values.insert(p.name.clone(), default);
                                }
                                (false, _) => {
                                    let mut vals = vec![];
                                    for arg_str in ocrs.iter() {
                                        match ParamValue::from_arg_str(ty, arg_str) {
                                            Ok(val) => vals.push(val),
                                            Err(e) => {
                                                return Err(getopts::Fail::UnexpectedArgument(
                                                    format!("{}, {}", p.name, e),
                                                ));
                                            }
                                        }
                                    }
                                    values.insert(p.name.clone(), ParamValue::Array(vals));
                                }
                            }
                        }
                    }
                }
                Ok(values)
            }
            Err(e) => Err(e),
        }
    }

    /// take parameter values and return parsed sql statement
    ///
    /// **NOTE** this method don't handle parameter wih default value
    /// so you should pass default value in context
    pub fn render<D: Dialect>(
        &self,
        dialect: &D,
        context: &HashMap<String, ParamValue>,
    ) -> Result<Vec<sqlparser::ast::Statement>, PSqlError> {
        let mut transformed = vec![];
        for t in self.tokens.iter() {
            match t {
                VariableToken::Var(var) => {
                    if let Some(val) = context.get(var) {
                        transformed.extend(val.clone().into_token(dialect))
                    } else {
                        return Err(PSqlError::MissingContextValue(var.clone()));
                    }
                }
                VariableToken::Normal(t) => transformed.push(t.clone()),
            }
        }
        log::info!(
            "{}",
            transformed
                .iter()
                .map(|t| t.to_string())
                .collect::<String>()
        );
        let mut parser = sqlparser::parser::Parser::new(transformed, dialect);
        let mut stmts = Vec::new();
        let mut expecting_statement_delimiter = false;
        loop {
            // ignore empty statements (between successive statement delimiters)
            while parser.consume_token(&Token::SemiColon) {
                expecting_statement_delimiter = false;
            }

            if parser.peek_token() == Token::EOF {
                break;
            }
            if expecting_statement_delimiter {
                return Err(PSqlError::ExpectEndOfStatement(parser.peek_token()));
            }

            let statement = parser.parse_statement().map_err(PSqlError::ParseError)?;
            stmts.push(statement);
            expecting_statement_delimiter = true;
        }
        Ok(stmts)
    }
}
