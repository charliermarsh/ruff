use rustpython_ast::{Constant, Expr, ExprKind};

use crate::ast::helpers::{create_expr, match_module_member, unparse_expr};
use crate::ast::types::Range;
use crate::autofix::Fix;
use crate::checkers::ast::Checker;
use crate::registry::{Diagnostic, RuleCode};
use crate::violations;

/// SIM112
pub fn use_capital_environment_variables(checker: &mut Checker, expr: &Expr) {
    // check `os.environ['foo']`
    if let ExprKind::Subscript { .. } = &expr.node {
        check_os_environ_subscript(checker, expr);
        return;
    }

    // check `os.environ.get('foo')` and `os.getenv('foo')``
    let is_os_environ_get = match_module_member(
        expr,
        "os.environ",
        "get",
        &checker.from_imports,
        &checker.import_aliases,
    );
    let is_os_getenv = match_module_member(
        expr,
        "os",
        "getenv",
        &checker.from_imports,
        &checker.import_aliases,
    );
    if !(is_os_environ_get || is_os_getenv) {
        return;
    }

    let ExprKind::Call { args, .. } = &expr.node else {
        return;
    };
    let Some(arg) = args.get(0) else {
        return;
    };
    let ExprKind::Constant { value: Constant::Str(env_var), kind } = &arg.node else {
        return;
    };
    let capital_env_var = env_var.to_ascii_uppercase();
    if &capital_env_var == env_var {
        return;
    }

    let mut diagnostic = Diagnostic::new(
        violations::UseCapitalEnvironmentVariables(capital_env_var.clone(), env_var.clone()),
        Range::from_located(arg),
    );
    if checker.patch(&RuleCode::SIM112) {
        let new_env_var = create_expr(ExprKind::Constant {
            value: capital_env_var.into(),
            kind: kind.clone(),
        });
        diagnostic.amend(Fix::replacement(
            unparse_expr(&new_env_var, checker.style),
            arg.location,
            arg.end_location.unwrap(),
        ));
    }
    checker.diagnostics.push(diagnostic);
}

fn check_os_environ_subscript(checker: &mut Checker, expr: &Expr) {
    let ExprKind::Subscript { value, slice, .. } = &expr.node else {
        return;
    };
    let ExprKind::Attribute { value: attr_value, attr, .. } = &value.node else {
        return;
    };
    let ExprKind::Name { id, .. } = &attr_value.node else {
        return;
    };
    if id != "os" || attr != "environ" {
        return;
    }
    let ExprKind::Constant { value: Constant::Str(env_var), kind } = &slice.node else {
        return;
    };
    let capital_env_var = env_var.to_ascii_uppercase();
    if &capital_env_var == env_var {
        return;
    }

    let mut diagnostic = Diagnostic::new(
        violations::UseCapitalEnvironmentVariables(capital_env_var.clone(), env_var.clone()),
        Range::from_located(slice),
    );
    if checker.patch(&RuleCode::SIM112) {
        let new_env_var = create_expr(ExprKind::Constant {
            value: capital_env_var.into(),
            kind: kind.clone(),
        });
        diagnostic.amend(Fix::replacement(
            unparse_expr(&new_env_var, checker.style),
            slice.location,
            slice.end_location.unwrap(),
        ));
    }
    checker.diagnostics.push(diagnostic);
}
