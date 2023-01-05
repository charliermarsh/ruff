use libcst_native::{Arg, Codegen, CodegenState, Expression};
use num_bigint::{BigInt, Sign};
use rustpython_ast::{Constant, Expr, ExprKind};
use rustpython_parser::lexer;
use rustpython_parser::lexer::Tok;
use regex::Regex;

use crate::autofix::Fix;
use crate::ast::types::Range;
use crate::checkers::ast::Checker;
use crate::checks::{Check, CheckKind};
use crate::cst::matchers::{match_call, match_expression};

/// Convert a python integer to a unsigned 32 but integer. We are assuming this will never overflow
/// because people will probbably never have more than 2^32 arguments to a format string. I am also
/// ignoring the signed, I personally checked and negative numbers are not allowed in format strings
fn convert_big_int(bigint: BigInt) -> Option<u32> {
    let (sign, digits) = bigint.to_u32_digits();
    match sign {
        Sign::Plus => digits.get(0).copied(),
        Sign::Minus => None,
        Sign::NoSign => Some(0),
    }
}

fn get_new_args(old_args: Vec<Arg>, correct_order: Vec<u32>) -> Vec<Arg> {
    let mut new_args: Vec<Arg> = Vec::new();
    for (i, given_idx) in correct_order.iter().enumerate() {
        // We need to keep the formatting in the same order but move the values
        let values = old_args.get(given_idx.to_owned() as usize).unwrap();
        let formatting = old_args.get(i).unwrap();
        let new_arg = Arg {
            value: values.value.clone(),
            comma: formatting.comma.clone(),
            // Kwargs are NOT allowed in .format (I checked)
            equal: None,
            keyword: None,
            star: values.star,
            whitespace_after_star: formatting.whitespace_after_star.clone(),
            whitespace_after_arg: formatting.whitespace_after_arg.clone(),
        };
        new_args.push(new_arg);
    }
    new_args
}

fn get_new_call(module_text: &str, correct_order: Vec<u32>) -> Option<String> {
    let mut expression = match match_expression(&module_text) {
        Err(_) => return None,
        Ok(item) => item,
    };
    let mut call = match match_call(&mut expression) {
        Err(_) => return None,
        Ok(item) => item,
    };
    call.args = get_new_args(call.args.clone(), correct_order);
    // Create the new function
    if let Expression::Attribute(item) = &*call.func {
        let mut state = CodegenState::default();
        item.codegen(&mut state);
        let cleaned = remove_specifiers(&state.to_string());
        match match_expression(&cleaned) {
            Err(_) => return None,
            Ok(item) => call.func = Box::new(item),
        };
        // Create the string
        let mut final_state = CodegenState::default();
        expression.codegen(&mut final_state);
        return Some(final_state.to_string());
    }
    None
}

fn get_specifier_order(value_str: &Constant) -> Vec<u32> {
    let mut specifier_ints: Vec<u32> = vec![];
    if let Constant::Str(item) = value_str {
        // Whether the previous character was a Lbrace. If this is true and the next character is
        // an integer than this integer gets added to the list of constants
        let mut prev_l_brace = false;
        for (_, tok, _) in lexer::make_tokenizer(item).flatten() {
            if Tok::Lbrace == tok {
                prev_l_brace = true;
            } else if let Tok::Int { value } = tok {
                if prev_l_brace {
                    if let Some(int_val) = convert_big_int(value) {
                        specifier_ints.push(int_val);
                    }
                }
                prev_l_brace = false;
            } else {
                prev_l_brace = false;
            }
        }
    }
    specifier_ints
}

/// Returns a string without the format specifiers. Ex. "Hello {0} {1}" -> "Hello {} {}"
fn remove_specifiers(raw_specifiers: &str) -> String {
    let re = Regex::new(r"\{(\d+)\}").unwrap();
    re.replace_all(raw_specifiers, "{}").to_string()
}

/// UP029
pub fn format_specifiers(checker: &mut Checker, expr: &Expr, func: &Expr) {
    if let ExprKind::Attribute { value, attr, .. } = &func.node {
        if let ExprKind::Constant {
            value: cons_value, ..
        } = &value.node
        {
            if attr == "format" {
                let as_ints = get_specifier_order(cons_value);
                let call_range = Range::from_located(expr);
                let call_text = checker.locator.slice_source_code_range(&call_range);
                let new_call = match get_new_call(&call_text, as_ints) {
                    None => return,
                    Some(item) => item,
                };
                println!("{}", new_call);
                let mut check = Check::new(CheckKind::FormatSpecifiers, Range::from_located(expr));
                if checker.patch(check.kind.code()) {
                    check.amend(Fix::replacement(
                        new_call,
                        expr.location,
                        expr.end_location.unwrap(),
                    ));
                }
                checker.add_check(check);
            }
        }
    }
}