use anyhow::{bail, Result};
use libcst_native::{
    BooleanOp, BooleanOperation, Codegen, CodegenState, CompoundStatement, Expression, If,
    LeftParen, ParenthesizableWhitespace, ParenthesizedNode, RightParen, SimpleWhitespace,
    Statement, Suite,
};

use crate::ast::types::Range;
use crate::autofix::Fix;
use crate::cst::matchers::match_module;
use crate::source_code_locator::SourceCodeLocator;

fn parenthesize_and_operand(expr: Expression) -> Expression {
    match expr {
        Expression::BooleanOperation(boolean_operation)
            if matches!(boolean_operation.operator, BooleanOp::Or { .. }) =>
        {
            Expression::BooleanOperation(
                boolean_operation.with_parens(LeftParen::default(), RightParen::default()),
            )
        }
        Expression::IfExp(if_exp) => {
            Expression::IfExp(if_exp.with_parens(LeftParen::default(), RightParen::default()))
        }
        Expression::Lambda(lambda) => {
            Expression::Lambda(lambda.with_parens(LeftParen::default(), RightParen::default()))
        }
        Expression::NamedExpr(named_expr) => Expression::NamedExpr(
            named_expr.with_parens(LeftParen::default(), RightParen::default()),
        ),
        _ => expr,
    }
}

/// (SIM102) Convert `if a: if b:` to `if a and b:`.
pub(crate) fn fix_nested_if_statements(
    locator: &SourceCodeLocator,
    stmt: &rustpython_ast::Stmt,
) -> Result<Fix> {
    let module_text = locator.slice_source_code_range(&Range::from_located(stmt));
    let mut tree = match_module(&module_text)?;

    let [Statement::Compound(CompoundStatement::If(outer_if))] = &mut *tree.body else {
        bail!("Expected one outer if statement")
    };

    let If {
        body: Suite::IndentedBlock(ref mut outer_body),
        orelse: None,
        ..
    } = outer_if else {
        bail!("Expected outer if to have indented body and no else")
    };

    let [Statement::Compound(CompoundStatement::If(inner_if @ If { orelse: None, .. }))] =
        &mut *outer_body.body
    else {
        bail!("Expected one inner if statement");
    };

    outer_if.test = Expression::BooleanOperation(Box::new(BooleanOperation {
        left: Box::new(parenthesize_and_operand(outer_if.test.clone())),
        operator: BooleanOp::And {
            whitespace_before: ParenthesizableWhitespace::SimpleWhitespace(SimpleWhitespace(" ")),
            whitespace_after: ParenthesizableWhitespace::SimpleWhitespace(SimpleWhitespace(" ")),
        },
        right: Box::new(parenthesize_and_operand(inner_if.test.clone())),
        lpar: vec![],
        rpar: vec![],
    }));
    outer_if.body = inner_if.body.clone();

    let mut state = CodegenState::default();
    tree.codegen(&mut state);
    Ok(Fix::replacement(
        state.to_string(),
        stmt.location,
        stmt.end_location.unwrap(),
    ))
}
