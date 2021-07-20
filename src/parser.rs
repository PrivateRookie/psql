use std::{
    collections::{HashMap, HashSet},
    process::exit,
    str::FromStr,
};

use nom::{
    branch::alt,
    bytes::complete::{is_not, tag, take_till, take_while},
    character::complete::{alpha1, alphanumeric1, char},
    combinator::{cut, map, opt, recognize, verify},
    error::context,
    multi::{many0, separated_list0},
    number::complete::double as nom_double,
    sequence::{self, pair, preceded, terminated, tuple},
    IResult,
};
use sqlparser::{dialect::Dialect, test_utils::number, tokenizer::{Token, Whitespace}};
use thiserror::Error;

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
                    "[{}]",
                    arr.iter()
                        .map(|i| i.to_string())
                        .collect::<Vec<String>>()
                        .join(", ")
                )
            }
        }
    }
}

// impl FromStr for ParamValue {
//     type Err = String;

//     fn from_str(s: &str) -> Result<Self, Self::Err> {
//         let (remaining, val) = parse_default(s).map_err(|e| e.to_string())?;
//         if !remaining.is_empty() {
//             Err(format!(
//                 "invalid value, {} with remaining {}",
//                 val.to_string(),
//                 remaining
//             ))
//         } else {
//             Ok(val)
//         }
//     }
// }

#[derive(Debug, PartialEq, Eq)]
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

#[derive(Debug, PartialEq, Eq)]
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

#[derive(Debug, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: ParamTy,
    pub default: Option<ParamValue>,
    pub help: String,
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("invalid variable, expect identifier, found {0}")]
    InvalidVariable(Token),
    #[error("unused params {0:?}")]
    UnusedParams(HashSet<String>),
    #[error("missing params {0:?}")]
    MissingParams(HashSet<String>),
    #[error("duplicated param {0}")]
    DuplicatedParam(String),
}

fn double_quote_str(input: &str) -> IResult<&str, &str> {
    let not_quote_slash = is_not("\"\\");
    context(
        "double quote str",
        map(
            tuple((tag("\""), not_quote_slash, tag("\""))),
            |(_, str, _)| str,
        ),
    )(input)
}

fn single_quote_str(input: &str) -> IResult<&str, &str> {
    let not_quote_slash = is_not("'\\");
    context(
        "single quote str",
        map(
            tuple((tag("'"), not_quote_slash, tag("'"))),
            |(_, str, _)| str,
        ),
    )(input)
}

fn str(input: &str) -> IResult<&str, ParamValue> {
    context(
        "str",
        map(alt((single_quote_str, double_quote_str)), |val: &str| {
            ParamValue::Str(val.to_string())
        }),
    )(input)
}

fn double(input: &str) -> IResult<&str, ParamValue> {
    context("double", map(nom_double, |val| ParamValue::Num(val)))(input)
}

fn raw(input: &str) -> IResult<&str, ParamValue> {
    let not_quote_slash = is_not("#\\");
    context(
        "raw val",
        map(
            tuple((tag("#"), not_quote_slash, tag("#"))),
            |(_, str, _): (&str, &str, &str)| ParamValue::Raw(str.to_string()),
        ),
    )(input)
}

fn basic_val(input: &str) -> IResult<&str, ParamValue> {
    alt((str, raw, double))(input)
}

fn sp(input: &str) -> IResult<&str, &str> {
    let chars = " \t\r\n";
    take_while(move |c| chars.contains(c))(input)
}

fn no_newline_sp(input: &str) -> IResult<&str, &str> {
    let chars = " \t";
    take_while(move |c| chars.contains(c))(input)
}

fn parse_array(input: &str) -> IResult<&str, ParamValue> {
    // TODO should check type consistent
    context(
        "array",
        map(
            preceded(
                tuple((tag("["), no_newline_sp)),
                terminated(
                    separated_list0(tuple((no_newline_sp, tag(","), no_newline_sp)), basic_val),
                    tuple((no_newline_sp, tag("]"))),
                ),
            ),
            |val| ParamValue::Array(val),
        ),
    )(input)
}

fn parse_default(input: &str) -> IResult<&str, ParamValue> {
    alt((parse_array, basic_val))(input)
}

fn identifier(input: &str) -> IResult<&str, String> {
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

fn basic_ty(input: &str) -> IResult<&str, InnerTy> {
    context(
        "basic ty",
        alt((
            map(tag("str"), |_| InnerTy::Str),
            map(tag("num"), |_| InnerTy::Num),
            map(tag("raw"), |_| InnerTy::Raw),
        )),
    )(input)
}

fn parse_ty(input: &str) -> IResult<&str, ParamTy> {
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
        map(basic_ty, |ty| ParamTy::Basic(ty)),
    ))(input)
}

/// parse param line
///
/// format
/// --? <param_name>: <ty> [= <default>] [// <help message>]
fn param(input: &str) -> IResult<&str, Param> {
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
    let (input, default) = context(
        "default",
        opt(map(
            tuple((no_newline_sp, tag("="), no_newline_sp, parse_default)),
            |(_, _, _, default)| default,
        )),
    )(input)?;
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
    ];
    for (name, input) in cases.iter() {
        println!("[{}] {} -> {:?}", name, input, param(input));
    }
}

#[derive(Debug, PartialEq)]
pub enum VariableToken {
    Var(String),
    Normal(Token),
}

#[derive(Debug)]
pub struct Program {
    pub params: Vec<Param>,
    pub tokens: Vec<VariableToken>,
}

impl Program {
    pub fn tokenize(dialect: &impl Dialect, program: &str) -> Result<Program, ParseError> {
        let tokens = sqlparser::tokenizer::Tokenizer::new(dialect, program)
            .tokenize()
            .unwrap();
        let mut processed = vec![];
        let mut params = vec![];
        let mut expect_word = false;
        for token in tokens.into_iter() {
            match token {
                Token::AtSign => {
                    if expect_word {
                        return Err(ParseError::InvalidVariable(token));
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
                        if let Ok((_, param)) = param(&comment) {
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
                        return Err(ParseError::InvalidVariable(token));
                    } else {
                        processed.push(VariableToken::Normal(token))
                    }
                }
            }
        }
        let param_names_vec = params
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<String>>();
        let mut param_names = HashSet::new();
        for p in param_names_vec.into_iter() {
            if !param_names.insert(p.clone()) {
                return Err(ParseError::DuplicatedParam(p));
            }
        }
        let mut var_names = HashSet::new();
        for t in processed.iter() {
            if let VariableToken::Var(name) = t {
                var_names.insert(name.clone());
            }
        }
        let missing: HashSet<String> = var_names
            .difference(&param_names)
            .map(|v| v.clone())
            .collect();
        if !missing.is_empty() {
            return Err(ParseError::MissingParams(missing));
        }
        let unused: HashSet<String> = param_names
            .difference(&var_names)
            .map(|v| v.clone())
            .collect();
        if !unused.is_empty() {
            return Err(ParseError::UnusedParams(unused));
        }
        Ok(Program {
            tokens: processed,
            params,
        })
    }

    pub fn get_matches(&self) -> HashMap<String, ParamValue> {
        use getopts::Options;
        use std::env::args;
        let mut opts = Options::new();
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
            }
        }
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
                    let ocr: Option<String> = matches.opt_str(&p.name);
                    match (ocr, p.default.clone()) {
                        (None, None) => {
                            println!("missing required option {}\n{}", p.name, opts.usage("psql"));
                        },
                        (None, Some(default)) => {
                            values.insert(p.name.clone(), default);
                        },
                        (Some(val_str), _) => {
                            match &p.ty {
                                ParamTy::Basic(ty) => {
                                    match ty {
                                        InnerTy::Str => Ok(ParamValue::Str(val_str)),
                                        InnerTy::Num => double(&val_str).map(|(remain, val)| {
                                            
                                        }),
                                        InnerTy::Raw => todo!(),
                                    }
                                },
                                ParamTy::Array(_) => todo!(),
                            }
                        },
                    }
                }
                values
            }
            Err(e) => {
                println!("{}\n\n{}", e, opts.usage("psql"));
                exit(1);
            }
        }
    }
}
