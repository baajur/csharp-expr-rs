use crate::expressions::*;
use chrono::{prelude::*, Duration};
use num_format::{Locale, ToFormattedString};
use regex::Captures;
use regex::{Regex, RegexBuilder};
use std::collections::HashMap;
use std::rc::Rc;

fn exec_vec_is_null(params: &VecRcExpr, values: &IdentifierValues) -> Result<bool, String> {
    let len = params.len();
    if len == 0 {
        return Ok(true);
    }
    if len == 1 {
        return exec_expr_is_null(params.get(0).unwrap(), values);
    }
    Err("is_null only takes 0 or 1 parameter".to_string())
}

fn exec_expr_is_null(expr: &RcExpr, values: &IdentifierValues) -> Result<bool, String> {
    let res = exec_expr(expr, values)?;
    Ok(if let ExprResult::Null = res { true } else { false })
}

fn results_are_equals(left: &ExprResult, right: &ExprResult) -> bool {
    if let ExprResult::Null = *left {
        return false;
    }

    if let ExprResult::Null = *right {
        return false;
    }

    left == right
}

fn result_to_string(expr: &ExprResult) -> Result<String, String> {
    if expr.is_final() {
        Ok(expr.to_string())
    } else {
        Err("Can't change this expression to string".to_string())
    }
}

fn exec_expr_to_string(expr: &RcExpr, values: &IdentifierValues) -> Result<String, String> {
    let res = exec_expr(expr, values)?;
    result_to_string(&res)
}

fn exec_expr_to_num(expr: &RcExpr, values: &IdentifierValues, decimal_separator: Option<char>) -> Result<ExprDecimal, String> {
    let res = exec_expr(expr, values)?;
    if let ExprResult::Num(n) = res {
        Ok(n)
    } else {
        let mut s = exec_expr_to_string(expr, values)?;
        if let Some(c) = decimal_separator {
            s = s.replace(c, ".")
        }
        let n: ExprDecimal = s.parse().or_else(|_| Err(format!("'{}' is not a number", s)))?;
        Ok(n)
    }
}

fn exec_expr_to_int(expr: &RcExpr, values: &IdentifierValues) -> Result<isize, String> {
    let res = exec_expr(expr, values)?;
    match &res {
        ExprResult::Num(n) => Ok(*n as isize),
        ExprResult::Str(s) => Ok(s.parse::<isize>().or_else(|_| Err(format!("'{}' is not a number", s)))?),
        expr => Err(format!("'{}' is not a number", expr)),
    }
}

fn exec_expr_to_bool(expr: &RcExpr, values: &IdentifierValues) -> Result<bool, String> {
    lazy_static! {
        static ref TRUE_STRING: Regex = RegexBuilder::new("^\\s*(true|1)\\s*$").case_insensitive(true).build().unwrap();
    }
    let res = exec_expr(expr, values)?;
    match &res {
        ExprResult::Boolean(b) => Ok(*b),
        ExprResult::Num(n) => Ok(*n == (1 as ExprDecimal)),
        ExprResult::Str(s) => Ok(TRUE_STRING.is_match(&*s)),
        _ => Err(format!("'{}' is not a boolean", expr)),
    }
}

fn exec_expr_to_date_no_defaults(expr: &RcExpr, values: &IdentifierValues) -> Result<NaiveDateTime, String> {
    exec_expr_to_date(expr, values, false, false, false, false, false, false)
}

fn exec_expr_to_date(
    expr: &RcExpr,
    values: &IdentifierValues,
    default_year: bool,
    default_month: bool,
    default_day: bool,
    default_hour: bool,
    default_minute: bool,
    default_second: bool,
) -> Result<NaiveDateTime, String> {
    let res = exec_expr(expr, values)?;
    let mut date_time = match &res {
        ExprResult::Date(d) => *d,
        e => {
            let text = result_to_string(&e)?;
            text.parse::<DateTime<Utc>>().map_err(|e| format!("{}", e))?.naive_utc()
        }
    };

    if default_year {
        date_time = date_time.with_year(1).unwrap();
    }
    if default_month {
        date_time = date_time.with_month(1).unwrap();
    }
    if default_day {
        date_time = date_time.with_day(1).unwrap();
    }
    if default_hour {
        date_time = date_time.with_hour(1).unwrap();
    }
    if default_minute {
        date_time = date_time.with_minute(1).unwrap();
    }
    if default_second {
        date_time = date_time.with_second(1).unwrap();
    }
    Ok(date_time)
}

fn assert_exact_params_count(params: &VecRcExpr, count: usize, f_name: &str) -> Result<(), String> {
    if params.len() == count {
        Ok(())
    } else {
        Err(format!("Function {} should have exactly {} parameters", f_name, count).to_string())
    }
}

fn assert_max_params_count(params: &VecRcExpr, count: usize, f_name: &str) -> Result<(), String> {
    if params.len() <= count {
        Ok(())
    } else {
        Err(format!("Function {} should have no more than {} parameters", f_name, count).to_string())
    }
}

fn assert_min_params_count(params: &VecRcExpr, count: usize, f_name: &str) -> Result<(), String> {
    if params.len() >= count {
        Ok(())
    } else {
        Err(format!("Function {} should have {} parameters or more", f_name, count).to_string())
    }
}

fn assert_between_params_count(params: &VecRcExpr, count_min: usize, count_max: usize, f_name: &str) -> Result<(), String> {
    let len = params.len();
    if len < count_min || len > count_max {
        Err(format!("Function {} should have between {} and {} parameters", f_name, count_min, count_max).to_string())
    } else {
        Ok(())
    }
}

/**********************************/
/*          Regex helpers         */
/**********************************/

fn make_case_insensitive_search_regex(search_pattern: &str) -> Result<Regex, String> {
    let search_pattern = regex::escape(&search_pattern);
    let regex = RegexBuilder::new(&search_pattern)
        .case_insensitive(true)
        .build()
        .map_err(|e| format!("{}", e))?;
    Ok(regex)
}

fn make_case_insensitive_equals_regex(search_pattern: &str) -> Result<Regex, String> {
    let search_pattern = regex::escape(&search_pattern);
    let search_pattern = format!("^{}$", search_pattern);
    let regex = RegexBuilder::new(&search_pattern)
        .case_insensitive(true)
        .build()
        .map_err(|e| format!("{}", e))?;
    Ok(regex)
}

fn like_pattern_to_regex_pattern(like_pattern: &str) -> String {
    let mut result = String::new();
    result.push('^');

    const ANY_MANY: &str = ".*";
    const ANY_ONE: &str = ".{1}";

    let mut previous_char = Option::<char>::default();
    for c in like_pattern.chars() {
        match (previous_char, c) {
            (None, '%') | (None, '_') => {
                previous_char = Some(c);
            }
            (None, _) => {
                result.push(c);
                previous_char = Some(c);
            }
            (Some('%'), '%') | (Some('_'), '_') => {
                result.push(c);
                previous_char = None;
            }
            (Some('%'), _) => {
                result.push_str(ANY_MANY);
                if c != '%' && c != '_' {
                    result.push(c);
                }
                previous_char = Some(c);
            }
            (Some('_'), _) => {
                result.push_str(ANY_ONE);
                if c != '%' && c != '_' {
                    result.push(c);
                }
                previous_char = Some(c);
            }
            (Some(_), '%') | (Some(_), '_') => {
                previous_char = Some(c);
            }
            (Some(_), _) => {
                result.push(c);
                previous_char = Some(c);
            }
        }
        dbg!("{} {} => {}", c, previous_char.unwrap_or(' '), &result);
    }

    match previous_char {
        None => {}
        Some('%') => result.push_str(ANY_MANY),
        Some('_') => result.push_str(ANY_ONE),
        _ => {}
    }

    result.push('$');
    result
}

fn make_case_insensitive_like_regex(search_pattern: &str) -> Result<Regex, String> {
    let search_pattern = regex::escape(&search_pattern);
    let regex_pattern = like_pattern_to_regex_pattern(&search_pattern);
    let regex = RegexBuilder::new(&regex_pattern)
        .case_insensitive(true)
        .build()
        .map_err(|e| format!("{}", e))?;
    Ok(regex)
}

/**********************************/
/*          Functions list        */
/**********************************/

pub fn get_functions() -> FunctionImplList {
    let mut funcs = FunctionImplList::new();
    funcs.insert("IsNull".to_string(), Rc::new(f_is_null));
    funcs.insert("IsBlank".to_string(), Rc::new(f_is_null));
    funcs.insert("AreEquals".to_string(), Rc::new(f_are_equals));
    funcs.insert("In".to_string(), Rc::new(f_in));
    funcs.insert("InLike".to_string(), Rc::new(f_in_like));
    funcs.insert("IsLike".to_string(), Rc::new(f_is_like));
    funcs.insert("Like".to_string(), Rc::new(f_is_like));
    funcs.insert("FirstNotNull".to_string(), Rc::new(f_first_not_null));
    funcs.insert("FirstNotEmpty".to_string(), Rc::new(f_first_not_null));
    funcs.insert("Concatenate".to_string(), Rc::new(f_concat));
    funcs.insert("Concat".to_string(), Rc::new(f_concat));
    funcs.insert("Exact".to_string(), Rc::new(f_exact));
    funcs.insert("Find".to_string(), Rc::new(f_find));
    funcs.insert("Substitute".to_string(), Rc::new(f_substitute));
    funcs.insert("Fixed".to_string(), Rc::new(f_fixed));
    funcs.insert("Left".to_string(), Rc::new(f_left));
    funcs.insert("Right".to_string(), Rc::new(f_right));
    funcs.insert("Mid".to_string(), Rc::new(f_mid));
    funcs.insert("Len".to_string(), Rc::new(f_len));
    funcs.insert("Lower".to_string(), Rc::new(f_lower));
    funcs.insert("Upper".to_string(), Rc::new(f_upper));
    funcs.insert("Trim".to_string(), Rc::new(f_trim));
    funcs.insert("FirstWord".to_string(), Rc::new(f_first_word));
    funcs.insert("FirstSentence".to_string(), Rc::new(f_first_sentence));
    funcs.insert("Capitalize".to_string(), Rc::new(f_capitalize));
    funcs.insert("Split".to_string(), Rc::new(f_split));
    funcs.insert("NumberValue".to_string(), Rc::new(f_number_value));
    funcs.insert("Text".to_string(), Rc::new(f_text));
    funcs.insert("StartsWith".to_string(), Rc::new(f_starts_with));
    funcs.insert("EndsWith".to_string(), Rc::new(f_ends_with));
    funcs.insert("ReplaceEquals".to_string(), Rc::new(f_replace_equals));
    funcs.insert("ReplaceLike".to_string(), Rc::new(f_replace_like));
    funcs.insert("And".to_string(), Rc::new(f_and));
    funcs.insert("Or".to_string(), Rc::new(f_or));
    funcs.insert("Not".to_string(), Rc::new(f_not));
    funcs.insert("Xor".to_string(), Rc::new(f_xor));
    funcs.insert("Iif".to_string(), Rc::new(f_iif));
    funcs.insert("If".to_string(), Rc::new(f_iif));
    funcs.insert("Abs".to_string(), Rc::new(f_abs));
    funcs.insert("Product".to_string(), Rc::new(f_product));
    funcs.insert("Sum".to_string(), Rc::new(f_sum));
    funcs.insert("Divide".to_string(), Rc::new(f_divide));
    funcs.insert("Subtract".to_string(), Rc::new(f_subtract));
    funcs.insert("Mod".to_string(), Rc::new(f_mod));
    funcs.insert("Modulo".to_string(), Rc::new(f_mod));
    funcs.insert("Round".to_string(), Rc::new(f_round));
    funcs.insert("GreaterThan".to_string(), Rc::new(f_greater_than));
    funcs.insert("Gt".to_string(), Rc::new(f_greater_than));
    funcs.insert("LowerThan".to_string(), Rc::new(f_lower_than));
    funcs.insert("Lt".to_string(), Rc::new(f_lower_than));
    funcs.insert("GreaterThanOrEqual".to_string(), Rc::new(f_greater_than_or_equal));
    funcs.insert("Gtoe".to_string(), Rc::new(f_greater_than_or_equal));
    funcs.insert("LowerThanOrEqual".to_string(), Rc::new(f_lower_than_or_equal));
    funcs.insert("Ltoe".to_string(), Rc::new(f_lower_than_or_equal));
    funcs.insert("Date".to_string(), Rc::new(f_date));
    funcs.insert("Now".to_string(), Rc::new(f_now));
    funcs.insert("Year".to_string(), Rc::new(f_year));
    funcs.insert("Month".to_string(), Rc::new(f_month));
    funcs.insert("Day".to_string(), Rc::new(f_day));
    funcs.insert("DateDiff".to_string(), Rc::new(f_date_diff));
    funcs.insert("DateDiffHours".to_string(), Rc::new(f_date_diff_hours));
    funcs.insert("DateDiffDays".to_string(), Rc::new(f_date_diff_days));
    funcs.insert("DateDiffMonths".to_string(), Rc::new(f_date_diff_months));
    funcs.insert("DateEquals".to_string(), Rc::new(f_date_equals));
    funcs.insert("DateNotEquals".to_string(), Rc::new(f_date_not_equals));
    funcs.insert("DateLower".to_string(), Rc::new(f_date_lower));
    funcs.insert("DateLowerOrEquals".to_string(), Rc::new(f_date_lower_or_equals));
    funcs.insert("DateGreater".to_string(), Rc::new(f_date_greater));
    funcs.insert("DateGreaterOrEquals".to_string(), Rc::new(f_date_greater_or_equals));
    funcs.insert("DateAddHours".to_string(), Rc::new(f_date_add_hours));
    funcs.insert("DateAddDays".to_string(), Rc::new(f_date_add_days));
    funcs.insert("DateAddMonths".to_string(), Rc::new(f_date_add_months));
    funcs.insert("DateAddYears".to_string(), Rc::new(f_date_add_years));
    funcs.insert("LocalDate".to_string(), Rc::new(f_local_date));
    funcs.insert("DateFormat".to_string(), Rc::new(f_date_format));
    funcs.insert("NowSpecificTimeZone".to_string(), Rc::new(f_now_specific_timezone));
    funcs.insert("Today".to_string(), Rc::new(f_today));
    funcs.insert("Time".to_string(), Rc::new(f_time));
    funcs
}

// #region Category names

// private const string MiscCatName = "Misc";
// private const string StringsCatName = "Strings";
// private const string LogicalCatName = "Logical";
// private const string MathCatName = "Math";
// private const string DateCatName = "DateTime";

// #endregion

/**********************************/
/*          Miscellaneous         */
/**********************************/

// IsNull, IsBlank
fn f_is_null(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    let res = exec_vec_is_null(params, values)?;
    Ok(ExprResult::Boolean(res))
}

// AreEquals
fn f_are_equals(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 2, "AreEquals")?;
    let left = exec_expr(params.get(0).unwrap(), values)?;
    let right = exec_expr(params.get(1).unwrap(), values)?;
    let res = results_are_equals(&left, &right);
    Ok(ExprResult::Boolean(res))
}

// In
fn f_in(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_min_params_count(params, 2, "In")?;
    let search = exec_expr(params.get(0).unwrap(), values)?;
    for p in params.iter().skip(1) {
        let p_result = exec_expr(p, values)?;
        if results_are_equals(&search, &p_result) {
            return Ok(ExprResult::Boolean(true));
        }
    }
    return Ok(ExprResult::Boolean(false));
}

// InLike
fn f_in_like(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_min_params_count(params, 2, "InLike")?;
    let search = exec_expr_to_string(params.get(0).unwrap(), values)?;
    let regex = make_case_insensitive_like_regex(&search)?;
    for p in params.iter().skip(1) {
        let text = exec_expr_to_string(p, values)?;
        if regex.is_match(&text) {
            return Ok(ExprResult::Boolean(true));
        }
    }
    Ok(ExprResult::Boolean(false))
}

// IsLike, Like
fn f_is_like(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 2, "IsLike")?;
    let text = exec_expr_to_string(params.get(0).unwrap(), values)?;
    let search = exec_expr_to_string(params.get(1).unwrap(), values)?;
    let regex = make_case_insensitive_like_regex(&search)?;
    Ok(ExprResult::Boolean(regex.is_match(&text)))
}

fn f_first_not_null(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    for p in params.iter() {
        let p_result = exec_expr(p, values)?;
        match p_result {
            ExprResult::Null => {}
            _ => return Ok(p_result),
        }
    }
    Ok(ExprResult::Null)
}

/**********************************/
/*          Strings               */
/**********************************/

// Concatenate, Concat
fn f_concat(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    let mut result = String::new();
    for p in params.iter() {
        let s = exec_expr_to_string(p, values)?;
        result.push_str(&s);
    }
    Ok(ExprResult::Str(result))
}

// Exact
fn f_exact(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 2, "Exact")?;
    let left = exec_expr_to_string(params.get(0).unwrap(), values)?;
    let right = exec_expr_to_string(params.get(1).unwrap(), values)?;
    Ok(ExprResult::Boolean(left == right))
}

// Find
fn f_find(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_between_params_count(params, 2, 3, "Find")?;
    let start_num: usize = match params.get(2) {
        None => 0,
        Some(epxr) => (exec_expr_to_int(epxr, values)? - 1).max(0) as usize,
    };

    let find_text = exec_expr_to_string(params.get(0).unwrap(), values)?;
    let regex = make_case_insensitive_search_regex(&find_text)?;

    let within_text = exec_expr_to_string(params.get(1).unwrap(), values)?;
    dbg!("{}", find_text);
    let position = match regex.find_at(&within_text, start_num) {
        None => 0,                // 0 for not found
        Some(m) => m.start() + 1, // because it's a Excel function and 1 based enumeration
    };
    Ok(ExprResult::Num(position as ExprDecimal))
}

// Substitute
fn f_substitute(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 3, "Substitute")?;

    let within_text = exec_expr_to_string(params.get(0).unwrap(), values)?;
    let find_text = exec_expr_to_string(params.get(1).unwrap(), values)?;
    let replace_text = exec_expr_to_string(params.get(2).unwrap(), values)?;

    let regex = make_case_insensitive_search_regex(&find_text)?;
    let replaced = regex.replace_all(&within_text, move |_c: &regex::Captures| replace_text.clone());

    Ok(ExprResult::Str(replaced.into()))
}

// Fixed
fn f_fixed(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_between_params_count(params, 1, 3, "Fixed")?;

    let number = exec_expr_to_num(params.get(0).unwrap(), values, None)?;

    let decimals = match params.get(1) {
        None => 2,
        Some(epxr) => exec_expr_to_int(epxr, values)?.max(0) as usize,
    };
    let no_commas = match params.get(2) {
        None => true,
        Some(epxr) => exec_expr_to_bool(epxr, values)?,
    };

    let result = if no_commas {
        format!("{num:.prec$}", num = number, prec = decimals)
    } else {
        let int = (number.trunc() as isize).to_formatted_string(&Locale::en);
        let fract = format!("{num:.prec$}", num = number.fract(), prec = decimals);
        let fract: Vec<&str> = fract.split(".").collect();
        let result = match fract.get(1) {
            Some(s) => format!("{}.{}", int, s),
            None => int,
        };
        result
    };
    Ok(ExprResult::Str(result))
}

// Left
fn f_left(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 2, "Left")?;
    let s = exec_expr_to_string(params.get(0).unwrap(), values)?;
    let size = exec_expr_to_int(params.get(1).unwrap(), values)?.max(0) as usize;
    if size == 0 {
        Ok(ExprResult::Str("".to_string()))
    } else if size >= s.len() {
        Ok(ExprResult::Str(s))
    } else {
        Ok(ExprResult::Str(format!("{}", &s[..size])))
    }
}

// Right
fn f_right(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 2, "Right")?;
    let s = exec_expr_to_string(params.get(0).unwrap(), values)?;
    let size = exec_expr_to_int(params.get(1).unwrap(), values)?.max(0) as usize;
    if size == 0 {
        Ok(ExprResult::Str("".to_string()))
    } else if size >= s.len() {
        Ok(ExprResult::Str(s))
    } else {
        Ok(ExprResult::Str(format!("{}", &s[s.len() - size..])))
    }
}

// Mid
fn f_mid(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 3, "Mid")?;
    let s = exec_expr_to_string(params.get(0).unwrap(), values)?;
    let false_position = exec_expr_to_int(params.get(1).unwrap(), values)?.max(1).min(s.len() as isize);
    let position = (false_position - 1) as usize;
    let size = exec_expr_to_int(params.get(2).unwrap(), values)?.max(0) as usize;
    if size == 0 {
        Ok(ExprResult::Str("".to_string()))
    } else if position == 0 && size >= s.len() {
        Ok(ExprResult::Str(s))
    } else {
        let end = (position + size).min(s.len());
        Ok(ExprResult::Str(format!("{}", &s[position..end])))
    }
}

fn single_string_func<F: FnOnce(String) -> ExprFuncResult>(params: &VecRcExpr, values: &IdentifierValues, f_name: &str, func: F) -> ExprFuncResult {
    assert_exact_params_count(params, 1, f_name)?;
    let s = exec_expr_to_string(params.get(0).unwrap(), values)?;
    func(s)
}

// Len
fn f_len(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    single_string_func(params, values, "Len", |s| Ok(ExprResult::Num(s.len() as ExprDecimal)))
}

// Lower
fn f_lower(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    single_string_func(params, values, "Lower", |s| Ok(ExprResult::Str(s.to_lowercase())))
}

// Upper
fn f_upper(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    single_string_func(params, values, "Upper", |s| Ok(ExprResult::Str(s.to_uppercase())))
}

// Trim
fn f_trim(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    single_string_func(params, values, "Trim", |s| Ok(ExprResult::Str(s.trim().to_string())))
}

fn is_punctuation(c: char) -> bool {
    c == '.' || c == ',' || c == '!' || c == '?' || c == '¿'
}
fn is_space(c: char) -> bool {
    c == ' ' || c == '\t' || c == '\r' || c == '\n'
}
fn is_sentence_punctuation(c: char) -> bool {
    c == '.' || c == '!' || c == '?'
}

// FirstWord
fn f_first_word(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    single_string_func(params, values, "FirstWord", |s| {
        let position = s.chars().position(|c| is_space(c) || is_punctuation(c));
        match position {
            None => Ok(ExprResult::Str(s)),
            Some(i) => Ok(ExprResult::Str(format!("{}", &s[..i]))),
        }
    })
}

// Text
fn f_text(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    single_string_func(params, values, "Text", |s| Ok(ExprResult::Str(s)))
}

// FirstSentence
fn f_first_sentence(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    single_string_func(params, values, "FirstSentence", |s| {
        let position = s.chars().position(|c| is_sentence_punctuation(c));
        match position {
            None => Ok(ExprResult::Str(s)),
            Some(i) => Ok(ExprResult::Str(format!("{}", &s[..i]))),
        }
    })
}

// Capitalize
fn f_capitalize(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    single_string_func(params, values, "Capitalize", |s| {
        todo!();
    })
}

// Split
fn f_split(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 3, "Split")?;
    let s = exec_expr_to_string(params.get(0).unwrap(), values)?;
    let separator = exec_expr_to_string(params.get(1).unwrap(), values)?;
    let index = exec_expr_to_int(params.get(2).unwrap(), values)?.max(0) as usize;
    let parts: Vec<&str> = s.split(&separator).collect();
    let result = match parts.get(index) {
        None => ExprResult::Null,
        Some(p) => ExprResult::Str(p.to_string()),
    };
    Ok(result)
}

// NumberValue
fn f_number_value(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_between_params_count(params, 1, 2, "NumberValue")?;
    let separator = match params.get(1) {
        None => None,
        Some(expr) => exec_expr_to_string(expr, values)?.chars().next(),
    };
    let number = exec_expr_to_num(params.get(0).unwrap(), values, separator)?;
    Ok(ExprResult::Num(number))
}

// StartsWith
fn f_starts_with(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 2, "StartsWith")?;
    let text = exec_expr_to_string(params.get(0).unwrap(), values)?;
    let search = exec_expr_to_string(params.get(1).unwrap(), values)?;

    let mut t_iter = text.chars().into_iter();
    let mut s_iter = search.chars().into_iter();

    loop {
        let t = t_iter.next();
        let s = s_iter.next();
        dbg!("{:?} {:?}", t, s);
        match (s, t) {
            (None, None) => return Ok(ExprResult::Boolean(true)),
            (None, Some(_)) => return Ok(ExprResult::Boolean(true)),
            (Some(_), None) => return Ok(ExprResult::Boolean(false)),
            (Some(vs), Some(vt)) => {
                if !vs.to_lowercase().eq(vt.to_lowercase()) {
                    return Ok(ExprResult::Boolean(false));
                }
            }
        }
    }
    unreachable!();
}

// EndsWith
fn f_ends_with(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 2, "EndsWith")?;
    let text = exec_expr_to_string(params.get(0).unwrap(), values)?;
    let search = exec_expr_to_string(params.get(1).unwrap(), values)?;

    let mut t_iter = text.chars().rev().into_iter();
    let mut s_iter = search.chars().rev().into_iter();

    loop {
        let t = t_iter.next();
        let s = s_iter.next();
        dbg!("{:?} {:?}", t, s);
        match (s, t) {
            (None, None) => return Ok(ExprResult::Boolean(true)),
            (None, Some(_)) => return Ok(ExprResult::Boolean(true)),
            (Some(_), None) => return Ok(ExprResult::Boolean(false)),
            (Some(vs), Some(vt)) => {
                if !vs.to_lowercase().eq(vt.to_lowercase()) {
                    return Ok(ExprResult::Boolean(false));
                }
            }
        }
    }
    unreachable!();
}

// ReplaceEquals
fn f_replace_equals(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_min_params_count(params, 4, "ReplaceEquals")?;
    if params.len() % 2 == 1 {
        return Err("Remplacement key/value parameters must come 2 by 2".to_string());
    }

    let text = exec_expr_to_string(params.get(0).unwrap(), values)?;
    let mut p_iter = params.iter().skip(2);
    loop {
        match (p_iter.next(), p_iter.next()) {
            (Some(pattern_expr), Some(replacement_expr)) => {
                let pattern = exec_expr_to_string(pattern_expr, values)?;
                let regex = make_case_insensitive_equals_regex(&pattern)?;

                if regex.is_match(&text) {
                    let replacement = exec_expr(replacement_expr, values);
                    return replacement;
                }
            }
            _ => break,
        }
    }

    let default = exec_expr(params.get(1).unwrap(), values);
    default
}

// ReplaceLike
fn f_replace_like(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_min_params_count(params, 4, "ReplaceLike")?;
    if params.len() % 2 == 1 {
        return Err("Remplacement key/value parameters must come 2 by 2".to_string());
    }

    let text = exec_expr_to_string(params.get(0).unwrap(), values)?;
    let mut p_iter = params.iter().skip(2);
    loop {
        match (p_iter.next(), p_iter.next()) {
            (Some(pattern_expr), Some(replacement_expr)) => {
                let pattern = exec_expr_to_string(pattern_expr, values)?;
                let regex = make_case_insensitive_like_regex(&pattern)?;

                if regex.is_match(&text) {
                    let replacement = exec_expr(replacement_expr, values);
                    return replacement;
                }
            }
            _ => break,
        }
    }

    let default = exec_expr(params.get(1).unwrap(), values);
    default
}

/**********************************/
/*          Logical               */
/**********************************/

// And
fn f_and(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    for expr in params {
        let b = exec_expr_to_bool(expr, values)?;
        if !b {
            return Ok(ExprResult::Boolean(false));
        }
    }
    Ok(ExprResult::Boolean(true))
}

// Or
fn f_or(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    for expr in params {
        let b = exec_expr_to_bool(expr, values)?;
        if b {
            return Ok(ExprResult::Boolean(true));
        }
    }
    Ok(ExprResult::Boolean(false))
}

// Not
fn f_not(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 1, "Not")?;
    Ok(ExprResult::Boolean(!exec_expr_to_bool(params.get(0).unwrap(), values)?))
}

// Xor
fn f_xor(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 2, "Xor")?;
    let p0 = exec_expr_to_bool(params.get(0).unwrap(), values)?;
    let p1 = exec_expr_to_bool(params.get(1).unwrap(), values)?;
    Ok(ExprResult::Boolean(p0 ^ p1))
}

// Iif, If
fn f_iif(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 3, "Iif")?;
    let test = exec_expr_to_bool(params.get(0).unwrap(), values)?;
    exec_expr(params.get(if test { 1 } else { 2 }).unwrap(), values)
}

/**********************************/
/*          Math                  */
/**********************************/

// Abs
fn f_abs(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 1, "Abs")?;
    let num = exec_expr_to_num(params.get(0).unwrap(), values, None)?;
    Ok(ExprResult::Num(num.abs()))
}

// Product
fn f_product(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    let mut result = 1 as ExprDecimal;
    for expr in params.iter() {
        let i = exec_expr_to_num(expr, values, None)?;
        let intermediate_result = result;
        result = std::panic::catch_unwind(|| intermediate_result * i)
            .map_err(|_| format!("Couldn't multiply {} by {} : overflow", result, i).to_string())?;
    }
    Ok(ExprResult::Num(result))
}

// Sum
fn f_sum(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    let mut result = 0 as ExprDecimal;
    for expr in params.iter() {
        let i = exec_expr_to_num(expr, values, None)?;
        let intermediate_result = result;
        result =
            std::panic::catch_unwind(|| intermediate_result + i).map_err(|_| format!("Couldn't add {} to {} : overflow", i, result).to_string())?;
    }
    Ok(ExprResult::Num(result))
}

// Divide
fn f_divide(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 2, "Divide")?;
    let num = exec_expr_to_num(params.get(0).unwrap(), values, None)?;
    let divisor = exec_expr_to_num(params.get(1).unwrap(), values, None)?;
    let result = std::panic::catch_unwind(|| num / divisor).map_err(|_| format!("Couldn't divide {} by {}", num, divisor).to_string())?;
    Ok(ExprResult::Num(result))
}

// Subtract
fn f_subtract(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 2, "Subtract")?;
    let num = exec_expr_to_num(params.get(0).unwrap(), values, None)?;
    let sub = exec_expr_to_num(params.get(1).unwrap(), values, None)?;
    let result = std::panic::catch_unwind(|| num - sub).map_err(|_| format!("Couldn't remove {} from {}", sub, num).to_string())?;
    Ok(ExprResult::Num(result))
}

// Mod, Modulo
fn f_mod(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 2, "Mod")?;
    let num = exec_expr_to_num(params.get(0).unwrap(), values, None)?;
    let divisor = exec_expr_to_num(params.get(1).unwrap(), values, None)?;
    let result = std::panic::catch_unwind(|| num % divisor).map_err(|_| format!("Couldn't module {} by {}", num, divisor).to_string())?;
    Ok(ExprResult::Num(result))
}

// Round
fn f_round(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 2, "Round")?;
    let num = exec_expr_to_num(params.get(0).unwrap(), values, None)?;
    let digits = exec_expr_to_int(params.get(1).unwrap(), values)?.max(0) as u32;
    let mult_div = (10 as u32).pow(digits) as ExprDecimal;
    let result = std::panic::catch_unwind(|| (num * mult_div).round() / mult_div)
        .map_err(|_| format!("Couldn't round {} to {} digits", num, digits).to_string())?;
    Ok(ExprResult::Num(result))
}

fn simple_operator<F: FnOnce(ExprDecimal, ExprDecimal) -> ExprFuncResult>(
    params: &VecRcExpr,
    values: &IdentifierValues,
    f_name: &str,
    func: F,
) -> ExprFuncResult {
    assert_exact_params_count(params, 2, f_name)?;
    let num_a = exec_expr_to_num(params.get(0).unwrap(), values, None)?;
    let num_b = exec_expr_to_num(params.get(1).unwrap(), values, None)?;
    func(num_a, num_b)
}

// GreaterThan, Gt
fn f_greater_than(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    simple_operator(params, values, "GreaterThan", |a, b| Ok(ExprResult::Boolean(a > b)))
}

// LowerThan, Lt
fn f_lower_than(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    simple_operator(params, values, "LowerThan", |a, b| Ok(ExprResult::Boolean(a < b)))
}

// GreaterThanOrEqual, Gtoe
fn f_greater_than_or_equal(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    simple_operator(params, values, "GreaterThanOrEqual", |a, b| Ok(ExprResult::Boolean(a >= b)))
}

// LowerThanOrEqual, Ltoe
fn f_lower_than_or_equal(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    simple_operator(params, values, "LowerThanOrEqual", |a, b| Ok(ExprResult::Boolean(a <= b)))
}

/**********************************/
/*          DateTime              */
/**********************************/

// Now
fn f_now(params: &VecRcExpr, _values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 0, "Now")?;
    Ok(ExprResult::Date(Utc::now().naive_utc()))
}

// Today
fn f_today(params: &VecRcExpr, _values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 0, "Today")?;
    let date = NaiveDateTime::new(Utc::now().date().naive_utc(), NaiveTime::from_hms(0, 0, 0));
    Ok(ExprResult::Date(date))
}

// Time
fn f_time(params: &VecRcExpr, _values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 0, "Time")?;
    let duration = Utc::now().time().signed_duration_since(NaiveTime::from_hms(0, 0, 0));
    Ok(ExprResult::TimeSpan(duration))
}

// NowSpecificTimeZone
fn f_now_specific_timezone(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_between_params_count(params, 0, 1, "NowSpecificTimeZone")?;

    let now = Utc::now();

    Ok(ExprResult::Date(match params.get(0) {
        None => now.naive_utc(),
        Some(expr) => {
            let time_zone_name = exec_expr_to_string(expr, values)?;
            let offset = get_utc_offset(&time_zone_name)?;
            let new_dt = now.with_timezone(offset);
            new_dt.naive_local()
        }
    }))
}

fn single_date_func<F: FnOnce(NaiveDateTime) -> ExprFuncResult>(
    params: &VecRcExpr,
    values: &IdentifierValues,
    f_name: &str,
    func: F,
) -> ExprFuncResult {
    assert_exact_params_count(params, 1, f_name)?;
    let date = exec_expr_to_date_no_defaults(params.get(0).unwrap(), values)?;
    func(date)
}

// Date
fn f_date(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    single_date_func(params, values, "Date", |d| Ok(ExprResult::Date(d)))
}

// Year
fn f_year(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    single_date_func(params, values, "Year", |d| Ok(ExprResult::Num(d.year() as ExprDecimal)))
}

// Month
fn f_month(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    single_date_func(params, values, "Month", |d| Ok(ExprResult::Num(d.month() as ExprDecimal)))
}

// Day
fn f_day(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    single_date_func(params, values, "Day", |d| Ok(ExprResult::Num(d.day() as ExprDecimal)))
}

fn two_dates_func_no_defaults<F: FnOnce(NaiveDateTime, NaiveDateTime) -> ExprFuncResult>(
    params: &VecRcExpr,
    values: &IdentifierValues,
    f_name: &str,
    func: F,
) -> ExprFuncResult {
    assert_exact_params_count(params, 2, f_name)?;
    let date_left = exec_expr_to_date_no_defaults(params.get(0).unwrap(), values)?;
    let date_right = exec_expr_to_date_no_defaults(params.get(1).unwrap(), values)?;
    func(date_left, date_right)
}

fn two_dates_func<F: FnOnce(NaiveDateTime, NaiveDateTime) -> ExprFuncResult>(
    params: &VecRcExpr,
    values: &IdentifierValues,
    f_name: &str,
    func: F,
) -> ExprFuncResult {
    assert_between_params_count(params, 2, 8, f_name)?;

    let default_year = params.get(2).map_or(Ok(false), |expr| exec_expr_to_bool(expr, values))?;
    let default_month = params.get(3).map_or(Ok(false), |expr| exec_expr_to_bool(expr, values))?;
    let default_day = params.get(4).map_or(Ok(false), |expr| exec_expr_to_bool(expr, values))?;
    let default_hour = params.get(5).map_or(Ok(false), |expr| exec_expr_to_bool(expr, values))?;
    let default_minute = params.get(6).map_or(Ok(false), |expr| exec_expr_to_bool(expr, values))?;
    let default_second = params.get(7).map_or(Ok(false), |expr| exec_expr_to_bool(expr, values))?;

    let date_left = exec_expr_to_date(
        params.get(0).unwrap(),
        values,
        default_year,
        default_month,
        default_day,
        default_hour,
        default_minute,
        default_second,
    )?;
    let date_right = exec_expr_to_date(
        params.get(1).unwrap(),
        values,
        default_year,
        default_month,
        default_day,
        default_hour,
        default_minute,
        default_second,
    )?;
    func(date_left, date_right)
}

// DateDiff
fn f_date_diff(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    two_dates_func_no_defaults(params, values, "DateDiff", |d1, d2| Ok(ExprResult::TimeSpan(d1 - d2)))
}

const SECONDS_IN_HOURS: ExprDecimal = 60 as ExprDecimal * 60 as ExprDecimal;
const SECONDS_IN_DAYS: ExprDecimal = SECONDS_IN_HOURS * 24 as ExprDecimal;
const SECONDS_IN_MONTHS: ExprDecimal = SECONDS_IN_DAYS * 30.5 as ExprDecimal;

//DateDiffHours
fn f_date_diff_hours(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    two_dates_func_no_defaults(params, values, "DateDiffHours", |d1, d2| {
        Ok(ExprResult::Num((d1 - d2).num_seconds() as ExprDecimal / SECONDS_IN_HOURS))
    })
}

// DateDiffDays
fn f_date_diff_days(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    two_dates_func_no_defaults(params, values, "DateDiffDays", |d1, d2| {
        Ok(ExprResult::Num((d1 - d2).num_seconds() as ExprDecimal / SECONDS_IN_DAYS))
    })
}

// DateDiffMonths
fn f_date_diff_months(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    two_dates_func_no_defaults(params, values, "DateDiffMonths", |d1, d2| {
        Ok(ExprResult::Num((d1 - d2).num_seconds() as ExprDecimal / SECONDS_IN_MONTHS))
    })
}

// DateEquals
fn f_date_equals(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    two_dates_func(params, values, "DateEquals", |d1, d2| Ok(ExprResult::Boolean(d1 == d2)))
}

// DateNotEquals
fn f_date_not_equals(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    two_dates_func(params, values, "DateNotEquals", |d1, d2| Ok(ExprResult::Boolean(d1 != d2)))
}

// DateLower
fn f_date_lower(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    two_dates_func(params, values, "DateLower", |d1, d2| Ok(ExprResult::Boolean(d1 < d2)))
}

// DateLowerOrEquals
fn f_date_lower_or_equals(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    two_dates_func(params, values, "DateLowerOrEquals", |d1, d2| Ok(ExprResult::Boolean(d1 <= d2)))
}

// DateGreater
fn f_date_greater(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    two_dates_func(params, values, "DateGreater", |d1, d2| Ok(ExprResult::Boolean(d1 > d2)))
}

// DateGreaterOrEquals
fn f_date_greater_or_equals(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    two_dates_func(params, values, "DateGreaterOrEquals", |d1, d2| Ok(ExprResult::Boolean(d1 >= d2)))
}

// DateAddHours
fn f_date_add_hours(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 2, "DateAddHours")?;
    let date_time = exec_expr_to_date_no_defaults(params.get(0).unwrap(), values)?;
    let hours = exec_expr_to_num(params.get(1).unwrap(), values, None)?;
    let date_time = date_time + Duration::seconds((hours * SECONDS_IN_HOURS) as i64);
    Ok(ExprResult::Date(date_time))
}

// DateAddDays
fn f_date_add_days(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 2, "DateAddDays")?;
    let date_time = exec_expr_to_date_no_defaults(params.get(0).unwrap(), values)?;
    let days = exec_expr_to_num(params.get(1).unwrap(), values, None)?;
    let date_time = date_time + Duration::seconds((days * SECONDS_IN_DAYS) as i64);
    Ok(ExprResult::Date(date_time))
}

// DateAddMonths
fn f_date_add_months(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 2, "DateAddMonths")?;
    let date_time = exec_expr_to_date_no_defaults(params.get(0).unwrap(), values)?;

    let months = exec_expr_to_int(params.get(1).unwrap(), values)?;
    let month0 = date_time.month0() as i32 + (months as i32);
    let mut years_to_add = month0 / 12;
    let mut new_month0 = month0 % 12;
    if new_month0 < 0 {
        new_month0 = new_month0 + 12;
        years_to_add = years_to_add - 1;
    }

    let mut new_date_time = date_time
        .with_year(date_time.year() + years_to_add)
        .ok_or(format!("Couldn't add {} years to the date {}", years_to_add, date_time))?;

    new_date_time =
        new_date_time
            .with_month0(new_month0 as u32)
            .ok_or(format!("Couldn't set {} as month to the date {}", new_month0 + 1, new_date_time))?;

    Ok(ExprResult::Date(new_date_time))
}

// DateAddYears
fn f_date_add_years(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_exact_params_count(params, 2, "DateAddYears")?;
    let date_time = exec_expr_to_date_no_defaults(params.get(0).unwrap(), values)?;
    let years = exec_expr_to_int(params.get(1).unwrap(), values)? as i32;

    let new_date_time = date_time
        .with_year(date_time.year() + years)
        .ok_or(format!("Couldn't add {} years to the date {}", years, date_time))?;

    Ok(ExprResult::Date(new_date_time))
}

// LocalDate
fn f_local_date(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_between_params_count(params, 1, 2, "LocalDate")?;
    let date_time = exec_expr_to_date_no_defaults(params.get(0).unwrap(), values)?;
    let time_zone_name = params
        .get(1)
        .map_or(Ok("Romance Standard Time".into()), |expr| exec_expr_to_string(expr, values))?;

    let offset = get_utc_offset(&time_zone_name)?;
    let new_dt = DateTime::<Local>::from_utc(date_time, *offset);
    Ok(ExprResult::Date(new_dt.naive_local()))
}

// DateFormat
fn f_date_format(params: &VecRcExpr, values: &IdentifierValues) -> ExprFuncResult {
    assert_between_params_count(params, 1, 2, "DateFormat")?;
    let date_time = exec_expr_to_date_no_defaults(params.get(0).unwrap(), values)?;
    let format = params
        .get(1)
        .map_or(Ok("yyyy-MM-dd HH:mm:ss.fff".into()), |expr| exec_expr_to_string(expr, values))?;

    let format = dotnet_format_to_strptime_format(&format);
    let result = date_time.format(&format);

    Ok(ExprResult::Str(result.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case("yyyy-MM-dd HH:mm:ss.fff" => "%Y-%m-%d %H:%M:%S.%3f")]
    fn test_dotnet_format_to_strptime_format(dotnet_format: &str) -> String {
        dotnet_format_to_strptime_format(dotnet_format)
    }

    #[test_case("abcd" => "^abcd$")]
    #[test_case("a_cd" => "^a.{1}cd$")]
    #[test_case("ab%d" => "^ab.*d$")]
    #[test_case("ab%%cd" => "^ab%cd$")]
    #[test_case("_abc" => "^.{1}abc$")]
    #[test_case("%abc" => "^.*abc$")]
    #[test_case("def_" => "^def.{1}$")]
    #[test_case("def%" => "^def.*$")]
    #[test_case("_O__%%___%%%O%" => "^.{1}O_%_.{1}%.*O.*$")]
    fn test_like_pattern_to_regex_pattern(like_pattern: &str) -> String {
        like_pattern_to_regex_pattern(like_pattern)
    }
}

fn dotnet_format_to_strptime_format(dotnet_format: &str) -> String {
    lazy_static! {
        static ref REPLACEMENTS: [(Regex, &'static str); 46] = [
            (Regex::new("dddd").unwrap(), "%A"),
            (Regex::new("ddd").unwrap(), "%a"),
            (Regex::new("dd").unwrap(), "%DAY"),
            (Regex::new("d").unwrap(), "%e"),
            (Regex::new("%DAY").unwrap(), "%d"),
            // Ok it's scrappy but (?<!%)d => Error
            // look-around, including look-ahead and look-behind, is not supported
            (Regex::new("fffffff").unwrap(), "%7f"),
            (Regex::new("ffffff").unwrap(), "%6f"),
            (Regex::new("fffff").unwrap(), "%5f"),
            (Regex::new("ffff").unwrap(), "%4f"),
            (Regex::new("fff").unwrap(), "%3f"),
            (Regex::new("ff").unwrap(), "%2f"),
            // (Regex::new("f").unwrap(), "%1f"), // Not supporting this one, no one uses it anyway
            (Regex::new("FFFFFFF").unwrap(), "%7f"),
            (Regex::new("FFFFFF").unwrap(), "%6f"),
            (Regex::new("FFFFF").unwrap(), "%5f"),
            (Regex::new("FFFF").unwrap(), "%4f"),
            (Regex::new("FFF").unwrap(), "%3f"),
            (Regex::new("FF").unwrap(), "%2f"),
            (Regex::new("F").unwrap(), "%1f"),
            (Regex::new("hh").unwrap(), "%I"),
            (Regex::new("h").unwrap(), "%l"),
            (Regex::new("HH").unwrap(), "%_OURS"),
            (Regex::new("H").unwrap(), "%k"),
            (Regex::new("%_OURS").unwrap(), "%H"),
            (Regex::new("mm").unwrap(), "%_INUTE"),  // same, kind of unsupported
            (Regex::new("m").unwrap(), "%_INUTE"),   // same, kind of unsupported
            (Regex::new("MMMM").unwrap(), "%B"),
            (Regex::new("MMM").unwrap(), "%b"),
            (Regex::new("MM").unwrap(), "%m"),
            (Regex::new("M").unwrap(), "%m"),
            (Regex::new("%_INUTE").unwrap(), "%M"),
            (Regex::new("%_INUTE").unwrap(), "%M"),
            (Regex::new("ss").unwrap(), "%S"),
            (Regex::new("s").unwrap(), "%S"),
            (Regex::new("tt").unwrap(), "%P"),
            (Regex::new("t").unwrap(), "%P"),
            (Regex::new("yyyyy").unwrap(), "%Y"),
            (Regex::new("yyyy").unwrap(), "%Y"),
            (Regex::new("yyy").unwrap(), "%Y"),
            (Regex::new("yy").unwrap(), "%YEAR"),
            (Regex::new("y").unwrap(), "%y"),
            (Regex::new("%YEAR").unwrap(), "%y"),
            (Regex::new("zzz").unwrap(), "%:_one"),
            (Regex::new("zz").unwrap(), "%_one"),
            (Regex::new("z").unwrap(), "%z"),
            (Regex::new("%_one").unwrap(), "%z"),
            (Regex::new("%:_one").unwrap(), "%:z"),
        ];
    }

    let result = REPLACEMENTS.iter().fold(dotnet_format.to_string(), |acc, replacer| {
        // let res = replacer.0.replace(&acc, replacer.1).to_string();
        // println!("{}", res);
        // res
        replacer.0.replace(&acc, replacer.1).to_string()
    });

    result
}

// Could be replaced by ? https://github.com/chronotope/chrono-tz/
fn get_utc_offset(time_zone_name: &str) -> Result<&'static FixedOffset, String> {
    lazy_static! {
        static ref TIME_ZONES: HashMap<&'static str, FixedOffset> = {
            let mut m = HashMap::new();
            m.insert("Dateline Standard Time", FixedOffset::west(43200));
            m.insert("UTC-11", FixedOffset::west(39600));
            m.insert("Aleutian Standard Time", FixedOffset::west(36000));
            m.insert("Hawaiian Standard Time", FixedOffset::west(36000));
            m.insert("Marquesas Standard Time", FixedOffset::west(34200));
            m.insert("Alaskan Standard Time", FixedOffset::west(32400));
            m.insert("UTC-09", FixedOffset::west(32400));
            m.insert("Pacific Standard Time (Mexico)", FixedOffset::west(28800));
            m.insert("UTC-08", FixedOffset::west(28800));
            m.insert("Pacific Standard Time", FixedOffset::west(28800));
            m.insert("US Mountain Standard Time", FixedOffset::west(25200));
            m.insert("Mountain Standard Time (Mexico)", FixedOffset::west(25200));
            m.insert("Mountain Standard Time", FixedOffset::west(25200));
            m.insert("Central America Standard Time", FixedOffset::west(21600));
            m.insert("Central Standard Time", FixedOffset::west(21600));
            m.insert("Easter Island Standard Time", FixedOffset::west(21600));
            m.insert("Central Standard Time (Mexico)", FixedOffset::west(21600));
            m.insert("Canada Central Standard Time", FixedOffset::west(21600));
            m.insert("SA Pacific Standard Time", FixedOffset::west(18000));
            m.insert("Eastern Standard Time (Mexico)", FixedOffset::west(18000));
            m.insert("Eastern Standard Time", FixedOffset::west(18000));
            m.insert("Haiti Standard Time", FixedOffset::west(18000));
            m.insert("Cuba Standard Time", FixedOffset::west(18000));
            m.insert("US Eastern Standard Time", FixedOffset::west(18000));
            m.insert("Turks And Caicos Standard Time", FixedOffset::west(18000));
            m.insert("Paraguay Standard Time", FixedOffset::west(14400));
            m.insert("Atlantic Standard Time", FixedOffset::west(14400));
            m.insert("Venezuela Standard Time", FixedOffset::west(14400));
            m.insert("Central Brazilian Standard Time", FixedOffset::west(14400));
            m.insert("SA Western Standard Time", FixedOffset::west(14400));
            m.insert("Pacific SA Standard Time", FixedOffset::west(14400));
            m.insert("Newfoundland Standard Time", FixedOffset::west(12600));
            m.insert("Tocantins Standard Time", FixedOffset::west(10800));
            m.insert("E. South America Standard Time", FixedOffset::west(10800));
            m.insert("SA Eastern Standard Time", FixedOffset::west(10800));
            m.insert("Argentina Standard Time", FixedOffset::west(10800));
            m.insert("Greenland Standard Time", FixedOffset::west(10800));
            m.insert("Montevideo Standard Time", FixedOffset::west(10800));
            m.insert("Magallanes Standard Time", FixedOffset::west(10800));
            m.insert("Saint Pierre Standard Time", FixedOffset::west(10800));
            m.insert("Bahia Standard Time", FixedOffset::west(10800));
            m.insert("UTC-02", FixedOffset::west(7200));
            m.insert("Mid-Atlantic Standard Time", FixedOffset::west(7200));
            m.insert("Azores Standard Time", FixedOffset::west(3600));
            m.insert("Cape Verde Standard Time", FixedOffset::west(3600));
            m.insert("UTC", FixedOffset::east(0));
            m.insert("GMT Standard Time", FixedOffset::east(0));
            m.insert("Greenwich Standard Time", FixedOffset::east(0));
            m.insert("Sao Tome Standard Time", FixedOffset::east(0));
            m.insert("Morocco Standard Time", FixedOffset::east(0));
            m.insert("W. Europe Standard Time", FixedOffset::east(3600));
            m.insert("Central Europe Standard Time", FixedOffset::east(3600));
            m.insert("Romance Standard Time", FixedOffset::east(3600));
            m.insert("Central European Standard Time", FixedOffset::east(3600));
            m.insert("W. Central Africa Standard Time", FixedOffset::east(3600));
            m.insert("Jordan Standard Time", FixedOffset::east(7200));
            m.insert("GTB Standard Time", FixedOffset::east(7200));
            m.insert("Middle East Standard Time", FixedOffset::east(7200));
            m.insert("Egypt Standard Time", FixedOffset::east(7200));
            m.insert("E. Europe Standard Time", FixedOffset::east(7200));
            m.insert("Syria Standard Time", FixedOffset::east(7200));
            m.insert("West Bank Standard Time", FixedOffset::east(7200));
            m.insert("South Africa Standard Time", FixedOffset::east(7200));
            m.insert("FLE Standard Time", FixedOffset::east(7200));
            m.insert("Israel Standard Time", FixedOffset::east(7200));
            m.insert("Kaliningrad Standard Time", FixedOffset::east(7200));
            m.insert("Sudan Standard Time", FixedOffset::east(7200));
            m.insert("Libya Standard Time", FixedOffset::east(7200));
            m.insert("Namibia Standard Time", FixedOffset::east(7200));
            m.insert("Arabic Standard Time", FixedOffset::east(10800));
            m.insert("Turkey Standard Time", FixedOffset::east(10800));
            m.insert("Arab Standard Time", FixedOffset::east(10800));
            m.insert("Belarus Standard Time", FixedOffset::east(10800));
            m.insert("Russian Standard Time", FixedOffset::east(10800));
            m.insert("E. Africa Standard Time", FixedOffset::east(10800));
            m.insert("Iran Standard Time", FixedOffset::east(12600));
            m.insert("Arabian Standard Time", FixedOffset::east(14400));
            m.insert("Astrakhan Standard Time", FixedOffset::east(14400));
            m.insert("Azerbaijan Standard Time", FixedOffset::east(14400));
            m.insert("Russia Time Zone 3", FixedOffset::east(14400));
            m.insert("Mauritius Standard Time", FixedOffset::east(14400));
            m.insert("Saratov Standard Time", FixedOffset::east(14400));
            m.insert("Georgian Standard Time", FixedOffset::east(14400));
            m.insert("Volgograd Standard Time", FixedOffset::east(14400));
            m.insert("Caucasus Standard Time", FixedOffset::east(14400));
            m.insert("Afghanistan Standard Time", FixedOffset::east(16200));
            m.insert("West Asia Standard Time", FixedOffset::east(18000));
            m.insert("Ekaterinburg Standard Time", FixedOffset::east(18000));
            m.insert("Pakistan Standard Time", FixedOffset::east(18000));
            m.insert("Qyzylorda Standard Time", FixedOffset::east(18000));
            m.insert("India Standard Time", FixedOffset::east(19800));
            m.insert("Sri Lanka Standard Time", FixedOffset::east(19800));
            m.insert("Nepal Standard Time", FixedOffset::east(20700));
            m.insert("Central Asia Standard Time", FixedOffset::east(21600));
            m.insert("Bangladesh Standard Time", FixedOffset::east(21600));
            m.insert("Omsk Standard Time", FixedOffset::east(21600));
            m.insert("Myanmar Standard Time", FixedOffset::east(23400));
            m.insert("SE Asia Standard Time", FixedOffset::east(25200));
            m.insert("Altai Standard Time", FixedOffset::east(25200));
            m.insert("W. Mongolia Standard Time", FixedOffset::east(25200));
            m.insert("North Asia Standard Time", FixedOffset::east(25200));
            m.insert("N. Central Asia Standard Time", FixedOffset::east(25200));
            m.insert("Tomsk Standard Time", FixedOffset::east(25200));
            m.insert("China Standard Time", FixedOffset::east(28800));
            m.insert("North Asia East Standard Time", FixedOffset::east(28800));
            m.insert("Singapore Standard Time", FixedOffset::east(28800));
            m.insert("W. Australia Standard Time", FixedOffset::east(28800));
            m.insert("Taipei Standard Time", FixedOffset::east(28800));
            m.insert("Ulaanbaatar Standard Time", FixedOffset::east(28800));
            m.insert("Aus Central W. Standard Time", FixedOffset::east(31500));
            m.insert("Transbaikal Standard Time", FixedOffset::east(32400));
            m.insert("Tokyo Standard Time", FixedOffset::east(32400));
            m.insert("North Korea Standard Time", FixedOffset::east(32400));
            m.insert("Korea Standard Time", FixedOffset::east(32400));
            m.insert("Yakutsk Standard Time", FixedOffset::east(32400));
            m.insert("Cen. Australia Standard Time", FixedOffset::east(34200));
            m.insert("AUS Central Standard Time", FixedOffset::east(34200));
            m.insert("E. Australia Standard Time", FixedOffset::east(36000));
            m.insert("AUS Eastern Standard Time", FixedOffset::east(36000));
            m.insert("West Pacific Standard Time", FixedOffset::east(36000));
            m.insert("Tasmania Standard Time", FixedOffset::east(36000));
            m.insert("Vladivostok Standard Time", FixedOffset::east(36000));
            m.insert("Lord Howe Standard Time", FixedOffset::east(37800));
            m.insert("Bougainville Standard Time", FixedOffset::east(39600));
            m.insert("Russia Time Zone 10", FixedOffset::east(39600));
            m.insert("Magadan Standard Time", FixedOffset::east(39600));
            m.insert("Norfolk Standard Time", FixedOffset::east(39600));
            m.insert("Sakhalin Standard Time", FixedOffset::east(39600));
            m.insert("Central Pacific Standard Time", FixedOffset::east(39600));
            m.insert("Russia Time Zone 11", FixedOffset::east(43200));
            m.insert("New Zealand Standard Time", FixedOffset::east(43200));
            m.insert("UTC+12", FixedOffset::east(43200));
            m.insert("Fiji Standard Time", FixedOffset::east(43200));
            m.insert("Kamchatka Standard Time", FixedOffset::east(43200));
            m.insert("Chatham Islands Standard Time", FixedOffset::east(45900));
            m.insert("UTC+13", FixedOffset::east(46800));
            m.insert("Tonga Standard Time", FixedOffset::east(46800));
            m.insert("Samoa Standard Time", FixedOffset::east(46800));
            m.insert("Line Islands Standard Time", FixedOffset::east(50400));
            m
        };
    };

    if let Some(time_zone) = TIME_ZONES.get(time_zone_name) {
        Ok(time_zone)
    } else {
        Err(format!("Unable to find a time zone named '{}'", time_zone_name))
    }
}
