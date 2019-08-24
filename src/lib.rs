// #![cfg(feature = "alloc")]

// extern crate nom;
// extern crate jemallocator;

// #[global_allocator]
// static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

// #![feature(result_map_or_else)]
#![deny(bare_trait_objects)]

//external crates
use std::cmp;
use std::ffi::{CStr, CString};
use std::fmt;
use std::os::raw::c_char;
use std::rc::Rc;
use std::slice;
use std::vec::Vec;
// use libc::{c_char, uint32_t};
// use std::ffi::CStr;

use unescape::unescape;

use nom::{
    branch::alt,
    bytes::complete::{escaped, tag, take_while}, // escaped_transform
    character::complete::{alphanumeric0, alphanumeric1, char, one_of},
    combinator::{cut, map, map_opt, not, opt, recognize},
    error::{context, ErrorKind, ParseError},
    multi::separated_list,
    number::complete::double,
    sequence::{delimited, pair, preceded, terminated, tuple},
    IResult,
};
use std::collections::HashMap;
use std::str;

// got this list from rust : https://github.com/rust-lang/rust/blob/master/src/libsyntax/util/parser.rs
#[derive(PartialEq, Debug)]
pub enum AssocOp {
    /// `+`
    Add,
    /// `-`
    Subtract,
    /// `*`
    Multiply,
    /// `/`
    Divide,
    /// `%`
    Modulus,
    /// `&&`
    LAnd,
    /// `||`
    LOr,
    /// `==`
    Equal,
    /// `<`
    Less,
    /// `<=`
    LessEqual,
    /// `!=`
    NotEqual,
    /// `>`
    Greater,
    /// `>=`
    GreaterEqual,
}

#[repr(C)]
#[derive(Clone)]
pub enum Expr {
    Str(String),
    Boolean(bool),
    Num(f64),
    Array(Vec<Expr>),
    Identifier(String),
    FunctionCall(String, Vec<Expr>),
    PreparedFunctionCall(String, Vec<Expr>, Rc<FunctionImpl>),
    // BinaryOperator(Box<Expr>, Box<Expr>, AssocOp)
}

impl fmt::Debug for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Str(x) => write!(f, "Str({:?})", x),
            Expr::Boolean(x) => write!(f, "Boolean({:?})", x),
            Expr::Num(x) => write!(f, "Num({:?})", x),
            Expr::Array(x) => write!(f, "Array({:?})", x),
            Expr::Identifier(x) => write!(f, "Identifier({:?})", x),
            Expr::FunctionCall(s, x) => write!(f, "FunctionCall({:?},{:?})", s, x),
            Expr::PreparedFunctionCall(s, x, _) => {
                write!(f, "PreparedFunctionCall({:?},{:?})", s, x)
            }
        }
    }
}

impl cmp::PartialEq for Expr {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Expr::Str(x_a), Expr::Str(x_b)) => x_a == x_b,
            (Expr::Boolean(x_a), Expr::Boolean(x_b)) => x_a == x_b,
            (Expr::Num(x_a), Expr::Num(x_b)) => x_a == x_b,
            (Expr::Array(x_a), Expr::Array(x_b)) => x_a == x_b,
            (Expr::Identifier(x_a), Expr::Identifier(x_b)) => x_a == x_b,
            (Expr::FunctionCall(n_a, p_a), Expr::FunctionCall(n_b, p_b)) => {
                n_a == n_b && p_a == p_b
            }
            (Expr::PreparedFunctionCall(n_a, p_a, _), Expr::PreparedFunctionCall(n_b, p_b, _)) => {
                n_a == n_b && p_a == p_b
            }
            _ => false,
        }
    }
}

enum RefOrValue<'a> {
    Ref(&'a Expr),
    Value(Expr),
}

impl RefOrValue<'_> {
    fn get_ref(&self) -> &Expr {
        match self {
            RefOrValue::Ref(x) => x,
            RefOrValue::Value(x) => &x,
        }
    }
}

type FunctionImpl = dyn Fn(&Vec<Expr>) -> Result<Expr, String>;
type FunctionImplList = HashMap<String, Rc<FunctionImpl>>;

/// A nom parser has the following signature:
/// `Input -> IResult<Input, Output, Error>`, with `IResult` defined as:
/// `type IResult<I, O, E = (I, ErrorKind)> = Result<(I, O), Err<E>>;`

/// spaces combinator
fn sp<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, &'a str, E> {
    let chars = " \t\r\n";
    take_while(move |c| chars.contains(c))(input)
}

/// string interior combinator
fn parse_str<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, &'a str, E> {
    escaped(alphanumeric1, '\\', one_of("\\\"rnt"))(input)
}

/// boolean combinator
fn boolean<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, bool, E> {
    alt((map(tag("false"), |_| false), map(tag("true"), |_| true)))(input)
}

/// full string combinator
fn string<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, &'a str, E> {
    context(
        "string",
        preceded(char('\"'), cut(terminated(parse_str, char('\"')))),
    )(input)
}

/// array combinator
fn array<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, Vec<Expr>, E> {
    context(
        "array",
        preceded(
            char('['),
            cut(terminated(
                separated_list(preceded(sp, char(',')), value),
                preceded(sp, char(']')),
            )),
        ),
    )(input)
}

fn identifier<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, &'a str, E> {
    context(
        "identifier",
        preceded(
            opt(sp),
            preceded(
                opt(tag("@")),
                recognize(tuple((opt(tag("_")), alphanumeric0))),
            ),
        ),
    )(input)
}

/// parameters between parenthesis
fn parameters<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, Vec<Expr>, E> {
    context(
        "parameters",
        preceded(
            char('('),
            terminated(
                separated_list(preceded(sp, char(',')), value),
                // map_opt(opt(separated_list(preceded(opt(sp), char(',')), value)), |opt| opt),
                preceded(opt(sp), char(')')),
            ),
        ),
    )(input)
}

fn function_call<'a, E: ParseError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, (&'a str, Vec<Expr>), E> {
    pair(identifier, parameters)(input)
}

fn identifier_only<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, &'a str, E> {
    map(pair(identifier, not(parameters)), |(a, _b)| a)(input)
}

/// here, we apply the space parser before trying to parse a value
fn value<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, Expr, E> {
    preceded(
        sp,
        alt((
            map(double, Expr::Num),
            map(boolean, Expr::Boolean),
            map_opt(string, |s| {
                unescape(s).map(|unescaped| Expr::Str(String::from(unescaped)))
            }),
            map(function_call, |(f_name, params)| {
                Expr::FunctionCall(String::from(f_name), params)
            }),
            map(array, Expr::Array),
            map(identifier_only, |s| Expr::Identifier(s.to_string())),
        )),
    )(input)
}

fn expr<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, Expr, E> {
    delimited(opt(sp), value, opt(sp))(input)
}

fn parse_expr<'a>(expression: &'a str) -> Result<Expr, String> {
    let expr = expr::<(&str, ErrorKind)>(expression);
    match expr {
        Ok(ok) => Ok(ok.1),
        Err(err_kind) => Err(format!("{:?}", err_kind)),
    }
}

fn prepare_expr(expr: Expr, funcs: &FunctionImplList) -> Expr {
    if let Expr::FunctionCall(name, parameters) = expr {
        match &funcs.get(&name) {
            Some(fnc) => {
                let parameters = parameters
                    .into_iter()
                    .map(|p| prepare_expr(p, &funcs))
                    .collect();
                Expr::PreparedFunctionCall(name, parameters, Rc::clone(&fnc))
            }
            None => Expr::FunctionCall(name, parameters),
        }
    } else {
        expr
    }
}

fn exec_expr<'a>(
    ref_or_value: RefOrValue<'a>,
    values: &HashMap<String, String>,
) -> Result<RefOrValue<'a>, String> {
    match ref_or_value.get_ref() {
        Expr::Str(_) => Ok(ref_or_value),
        Expr::Boolean(_) => Ok(ref_or_value),
        Expr::Num(_) => Ok(ref_or_value),
        Expr::Array(_) => Ok(ref_or_value),
        Expr::Identifier(name) => match &values.get(name) {
            Some(s) => Ok(RefOrValue::Value(Expr::Str(s.to_string()))),
            None => Err(format!(
                "Unable to find value for identifier named '{}'",
                name
            )),
        },
        // Expr::BinaryOperator(_, _, _) => Ok(expr),
        Expr::FunctionCall(name, _parameters) => {
            Err(format!("Unable to find the function named '{}'", name))
        }
        Expr::PreparedFunctionCall(_, parameters, fnc) => {
            let call_result = fnc(parameters)?;
            exec_expr(RefOrValue::Value(call_result), values)
        }
    }
}

fn expr_to_string<'a>(expr: &'a Expr, values: &HashMap<String, String>) -> String {
    match expr {
        Expr::Str(s) => s.to_string(),
        Expr::Boolean(b) => b.to_string(),
        Expr::Num(n) => n.to_string(),
        Expr::Array(_) => "Array".to_string(),
        Expr::Identifier(i) => format!("[{}]", i),
        // Expr::Identifier(name) => match &values.get(name) {
        //     Some(s) => s.to_string(),
        //     None => format!("Unable to find value for identifier named '{}'", name),
        // },
        // Expr::BinaryOperator(_, _, _) => Ok(expr),
        Expr::FunctionCall(_, _) => "FunctionCall".to_string(),
        Expr::PreparedFunctionCall(_, _, _) => "PreparedFunctionCall".to_string(),
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;
    use test_case_derive::test_case;

    #[test]
    fn parse_parameters() {
        // let p = parameters::<ParseError<&str>>("");
        // assert_eq!(p.unwrap(), ("",Vec::<Expr>::new()));
        // println()
    }

    #[test_case("true" => Expr::Boolean(true))]
    #[test_case("false" => Expr::Boolean(false))]
    fn parse_boolean(expression: &str) -> Expr {
        parse_expr(expression).unwrap()
    }

    #[test_case(stringify!("test") => "test")]
    #[test_case(stringify!("test\"doublequote") => "test\"doublequote")]
    #[test_case(stringify!("test\\slash") => "test\\slash")]
    #[test_case(stringify!("test\newline") => "test\newline")]
    #[test_case(stringify!("test\ttab") => "test\ttab")]
    #[test_case(stringify!("test\rreturn") => "test\rreturn")]
    fn parse_str(expression: &str) -> String {
        let result = parse_expr(expression);
        println!("{:?}", result);
        let expr = result.unwrap();
        if let Expr::Str(result) = expr {
            result
        } else {
            panic!("{:?}", expr)
        }
    }

    #[test_case("1" => Expr::Num(1_f64))]
    #[test_case("1.2" => Expr::Num(1.2_f64))]
    #[test_case("-0.42" => Expr::Num(-0.42_f64))]
    fn parse_num(expression: &str) -> Expr {
        parse_expr(expression).unwrap()
    }

    #[test_case("id" => Expr::Identifier("id".to_string()))]
    #[test_case("@idarobase" => Expr::Identifier("idarobase".to_string()))]
    #[test_case("id_id" => Expr::Identifier("id_id".to_string()))]
    #[test_case("id42" => Expr::Identifier("id42".to_string()))]
    #[test_case("_id0" => Expr::Identifier("_id0".to_string()))]
    #[test_case("_id1" => Expr::Identifier("_id1".to_string()))]
    fn parse_identifier(expression: &str) -> Expr {
        parse_expr(expression).unwrap()
    }

    #[test_case("[1,2]" => Expr::Array(vec![Expr::Num(1_f64), Expr::Num(2_f64)]))]
    fn parse_array(expression: &str) -> Expr {
        parse_expr(expression).unwrap()
    }

    #[test_case("test(1,2)" => Expr::FunctionCall("test".to_string(), vec![Expr::Num(1_f64), Expr::Num(2_f64)]))]
    #[test_case("test()" => Expr::FunctionCall("test".to_string(), Vec::<Expr>::new()))]
    #[test_case("test(aa)" => Expr::FunctionCall("test".to_string(), vec![Expr::Identifier("aa".to_string())]))]
    fn parse_function_call(expression: &str) -> Expr {
        parse_expr(expression).unwrap()
    }

    #[test_case("test([\"value\", 42],2)" => Expr::FunctionCall("test".to_string(), vec![Expr::Array(vec!(Expr::Str("value".to_string()), Expr::Num(42_f64))), Expr::Num(2_f64)]))]
    fn parse_complexe_expressions(expression: &str) -> Expr {
        parse_expr(expression).unwrap()
    }

    #[test]
    fn execute_one_expression() {
        let mut funcs = FunctionImplList::new();
        funcs.insert(
            "first".to_string(),
            Rc::new(|v: &Vec<Expr>| {
                v.first().map_or_else(
                    || Err("There was no first value.".to_string()),
                    |x| Ok(x.clone()),
                )
            }),
        );

        funcs.insert(
            "forty_two".to_string(),
            Rc::new(|_v: &Vec<Expr>| Ok(Expr::Num(42_f64))),
        );
        funcs.insert(
            "forty_two_str".to_string(),
            Rc::new(|_v: &Vec<Expr>| Ok(Expr::Str("42".to_string()))),
        );

        let mut values = HashMap::<String, String>::new();
        values.insert("my".into(), "value".to_string());

        let expression = "first(first(first(my,2,3),2,3),2,3)";
        let result = parse_exec_expr(expression, &funcs, &values);
        assert_eq!(result, "value");
        println!("{:?}", result);
    }

    fn parse_exec_expr<'a>(
        expression: &'a str,
        funcs: &FunctionImplList,
        values: &HashMap<String, String>,
    ) -> String {
        let expr = parse_expr(expression).unwrap();
        let expr = prepare_expr(expr, funcs);
        let result = exec_expr(RefOrValue::Value(expr), values).unwrap();
        expr_to_string(&result.get_ref(), values)
    }
}

#[no_mangle]
pub extern "C" fn ffi_parse_expr(expression: *const c_char) -> *mut Expr {
    let c_str = unsafe {
        assert!(!expression.is_null());
        CStr::from_ptr(expression)
    };

    let r_str = c_str.to_str().unwrap();
    let expr = parse_expr(r_str).unwrap();

    let mut funcs: FunctionImplList = HashMap::new();

    funcs.insert(
        "true".to_string(),
        Rc::new(|_: &Vec<Expr>| Ok(Expr::Boolean(true))),
    );

    funcs.insert(
        "first".to_string(),
        Rc::new(|v: &Vec<Expr>| {
            v.first().map_or_else(
                || Err("There was no first value.".to_string()),
                |x| Ok(x.clone()),
            )
        }),
    );

    let expr = prepare_expr(expr, &funcs);

    let b = Box::new(expr);
    Box::into_raw(b)
}

#[repr(C)]
#[derive(Debug)]
pub struct IdentifierKeyValue {
    key: *const c_char,
    value: *const c_char,
}

#[no_mangle]
pub extern "C" fn ffi_exec_expr(
    ptr: *mut Expr,
    identifier_values: *const IdentifierKeyValue,
    identifier_values_len: usize,
) -> *mut c_char {
    let expr = unsafe {
        assert!(!ptr.is_null());
        &mut *ptr
    };

    let vals = unsafe {
        assert!(!identifier_values.is_null());
        slice::from_raw_parts(identifier_values, identifier_values_len)
    };

    let values: HashMap<_, _> = vals
        .into_iter()
        .map(|ikv| {
            let k = unsafe {
                assert!(!ikv.key.is_null());
                CStr::from_ptr(ikv.key)
            }
            .to_str()
            .unwrap()
            .to_string();

            let v = unsafe {
                assert!(!ikv.value.is_null());
                CStr::from_ptr(ikv.value)
            }
            .to_str()
            .unwrap()
            .to_string();

            (k, v)
        })
        .collect();

    // let values = HashMap::<String, String>::new();

    let result = exec_expr(RefOrValue::Ref(expr), &values).unwrap();
    let s_result = expr_to_string(&result.get_ref(), &values);

    let c_str_result = CString::new(s_result).unwrap();
    c_str_result.into_raw()
}

#[no_mangle]
pub extern "C" fn ffi_free_expr(ptr: *mut Expr) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        Box::from_raw(ptr);
    }
}

#[no_mangle]
pub extern "C" fn ffi_free_cstring(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe { CString::from_raw(ptr) };
}
