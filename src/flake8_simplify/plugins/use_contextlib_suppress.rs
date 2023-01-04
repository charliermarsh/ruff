use rustpython_ast::{Excepthandler, ExcepthandlerKind, Stmt};

use crate::ast::helpers;
use crate::ast::types::Range;
use crate::checkers::ast::Checker;
use crate::registry::{Check, CheckKind};

/// SIM105
pub fn use_contextlib_suppress(
    checker: &mut Checker,
    stmt: &Stmt,
    handlers: &[Excepthandler],
    orelse: &[Stmt],
) {
    if handlers.len() != 1 || !orelse.is_empty() {
        return;
    }
    let handler = &handlers[0];
    let ExcepthandlerKind::ExceptHandler { body, .. } = &handler.node;
    if body.len() == 1 {
        let node = &body[0].node;
        if matches!(node, rustpython_ast::StmtKind::Pass) {
            let handler_names: Vec<_> = helpers::extract_handler_names(handlers)
                .into_iter()
                .flatten()
                .collect();
            let exception = if handler_names.is_empty() {
                "Exception".to_string()
            } else {
                handler_names.join(", ")
            };
            let check = Check::new(
                CheckKind::UseContextlibSuppress(exception),
                Range::from_located(stmt),
            );
            checker.add_check(check);
        }
    }
}
