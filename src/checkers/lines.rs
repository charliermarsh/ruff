//! Lint rules based on checking raw physical lines.

use crate::checks::{Check, CheckCode};
use crate::pycodestyle::checks::{line_too_long, no_newline_at_end_of_file};
use crate::pygrep_hooks::plugins::blanket_type_ignore;
use crate::pyupgrade::checks::unnecessary_coding_comment;
use crate::settings::{flags, Settings};

pub fn check_lines(contents: &str, settings: &Settings, autofix: flags::Autofix) -> Vec<Check> {
    let mut checks: Vec<Check> = vec![];

    let enforce_unnecessary_coding_comment = settings.enabled.contains(&CheckCode::UP009);
    let enforce_line_too_long = settings.enabled.contains(&CheckCode::E501);
    let enforce_no_newline_at_end_of_file = settings.enabled.contains(&CheckCode::W292);
    let enforce_blanket_type_ignore = settings.enabled.contains(&CheckCode::PGH003);

    for (lineno, line) in contents.lines().enumerate() {
        // Enforce unnecessary coding comments (UP009).
        if enforce_unnecessary_coding_comment {
            if lineno < 2 {
                if let Some(check) = unnecessary_coding_comment(
                    lineno,
                    line,
                    matches!(autofix, flags::Autofix::Enabled)
                        && settings.fixable.contains(&CheckCode::UP009),
                ) {
                    checks.push(check);
                }
            }
        }

        // Enforce line length violations (E501).
        if enforce_line_too_long {
            if let Some(check) = line_too_long(lineno, line, settings.line_length) {
                checks.push(check);
            }
        }

        if enforce_blanket_type_ignore {
            if let Some(check) = blanket_type_ignore(lineno, line) {
                checks.push(check);
            }
        }
    }

    // Enforce newlines at end of files (W292).
    if enforce_no_newline_at_end_of_file {
        if let Some(check) = no_newline_at_end_of_file(contents) {
            checks.push(check);
        }
    }

    checks
}

#[cfg(test)]
mod tests {

    use super::check_lines;
    use crate::checks::CheckCode;
    use crate::settings::{flags, Settings};

    #[test]
    fn e501_non_ascii_char() {
        let line = "'\u{4e9c}' * 2"; // 7 in UTF-32, 9 in UTF-8.
        let check_with_max_line_length = |line_length: usize| {
            check_lines(
                line,
                &Settings {
                    line_length,
                    ..Settings::for_rule(CheckCode::E501)
                },
                flags::Autofix::Enabled,
            )
        };
        assert!(!check_with_max_line_length(6).is_empty());
        assert!(check_with_max_line_length(7).is_empty());
    }
}
