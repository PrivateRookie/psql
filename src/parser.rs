use nom::{
    branch::alt,
    bytes::complete::{is_not, tag, take_till, take_while},
    character::complete::{alpha1, alphanumeric1, char},
    combinator::{cut, map, recognize, verify},
    error::context,
    multi::{many0, separated_list0},
    number::complete::double as nom_double,
    sequence::{pair, preceded, terminated, tuple},
    IResult,
};

pub enum ParamValue {
    Str(String),
    Double(f64),
    Raw(String),
    Array(Vec<ParamValue>),
}

pub enum InnerTy {
    Str,
    Double,
    Raw,
}

pub enum ParamTy {
    Basic(InnerTy),
    Array(InnerTy),
}

pub struct Param {
    pub name: String,
    pub ty: ParamTy,
    pub default: Option<ParamValue>,
    pub help: String,
}

fn double_quote_str(input: &str) -> IResult<&str, &str> {
    let not_quote_slash = is_not("\"\\");
    verify(not_quote_slash, |s: &str| !s.is_empty())(input)
}

fn single_quote_str(input: &str) -> IResult<&str, &str> {
    let not_quote_slash = is_not("'\\");
    verify(not_quote_slash, |s: &str| !s.is_empty())(input)
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
    context("double", map(nom_double, |val| ParamValue::Double(val)))(input)
}

fn raw(input: &str) -> IResult<&str, ParamValue> {
    let not_quote_slash = is_not("#\\");
    context(
        "raw",
        map(
            verify(not_quote_slash, |s: &str| !s.is_empty()),
            |val: &str| ParamValue::Raw(val.to_string()),
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

fn parse_array(input: &str) -> IResult<&str, Vec<ParamValue>> {
    context(
        "array",
        preceded(
            char('['),
            cut(terminated(
                separated_list0(preceded(sp, char(',')), basic_val),
                preceded(sp, char(']')),
            )),
        ),
    )(input)
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
            map(tag("double"), |_| InnerTy::Double),
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
    let (input, _) = tag("? ")(input)?;
    let (input, name) = identifier(input)?;
    let (input, _) = take_till(|c| c == ':')(input)?;
    let (input, _) = take_while(|c| c == ' ' || c == '\t')(input)?;
    let (input, ty) = parse_ty(input)?;
    
    todo!()
}
