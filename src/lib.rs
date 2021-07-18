use nom::{
    branch::alt,
    bytes::complete::{tag, take_while_m_n},
    character::{
        complete::{alpha1, alphanumeric1},
        is_alphabetic, is_alphanumeric,
    },
    combinator::{map, recognize},
    error::{context, ContextError, ErrorKind, ParseError, VerboseError},
    multi::{many0, many_m_n},
    sequence::{pair, preceded},
    IResult,
};

mod parser;

#[derive(Debug, PartialEq, Eq)]
pub enum Token {
    Var(String),
    Common(String),
}

#[derive(Debug, PartialEq)]
pub struct VarIdent(String);

/// parse `@var_name` 
fn parse_var<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, VarIdent, E> {
    context(
        "parse identifer",
        map(
            preceded(
                context("need leading `@`", tag("@")),
                context(
                    "must starts with alpha or '_'",
                    recognize(pair(
                        alt((alpha1, tag("_"))),
                        many0(alt((alphanumeric1, tag("_")))),
                    )),
                ),
            ),
            |val: &str| VarIdent(val.to_string()),
        ),
    )(input)
}



#[test]
fn ident() {
    let source = "@1field";
    // assert_eq!(
    //     Ok(("", VarIdent("field".to_string()))),
    //     parse_var_ident(source)
    // );
    println!("{:#?}", parse_var::<VerboseError<&str>>(source))
}

// fn parse_var(input: &str) -> IResult<&str, Token> {
//     let (input, _) = preceded(tag("@"), second)
// }
