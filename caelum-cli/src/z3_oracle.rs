//! Native [`SmtOracle`] that shells out to the `z3` binary. This gives the CLI
//! a `--solver smtlib` path and, just as importantly, exercises the exact
//! SMT-LIB2 the browser sends to z3.js against a real Z3 — so the two paths are
//! kept honest by the same script generator.

use std::io::Write;
use std::process::{Command, Stdio};

use caelum_kernel::bmc::SmtOracle;
use caelum_kernel::diagnostics::{CaelumError, Result};

pub struct ProcessZ3Oracle;

impl SmtOracle for ProcessZ3Oracle {
    fn solve(&self, script: &str) -> Result<String> {
        let mut child = Command::new("z3")
            .arg("-in")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| CaelumError::Unsupported {
                message: format!("failed to spawn `z3` (is it on PATH?): {err}"),
            })?;

        child
            .stdin
            .take()
            .expect("piped stdin")
            .write_all(script.as_bytes())
            .map_err(|err| CaelumError::Model {
                message: format!("failed to write SMT-LIB2 to z3: {err}"),
            })?;

        let output = child.wait_with_output().map_err(|err| CaelumError::Model {
            message: format!("failed to read z3 output: {err}"),
        })?;

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use caelum_kernel::bmc::{check_with_bmc_using, BmcOptions, SmtScriptSolver};
    use caelum_kernel::checker::CheckStatus;
    use caelum_kernel::sema::check_source_file;
    use caelum_kernel::syntax::parse_source;

    use super::*;

    fn z3_available() -> bool {
        Command::new("z3")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn check_smtlib(source: &str, depth: usize) -> caelum_kernel::checker::CheckReport {
        let file = parse_source(source).expect("parse");
        check_source_file(&file).expect("sema");
        check_with_bmc_using(
            &file,
            &BmcOptions {
                depth,
                prove: false,
            },
            || Box::new(SmtScriptSolver::new(ProcessZ3Oracle)),
        )
        .expect("bmc via smtlib")
    }

    #[test]
    fn smtlib_matches_native_verdicts() {
        if !z3_available() {
            eprintln!("skipping: z3 binary not on PATH");
            return;
        }

        // Safety violation: □ (x != 2) must FAIL with a counterexample at x = 2.
        let report = check_smtlib(
            r"
            let x: 0..2
            init { x = 0 }
            transition step { x' = (x + 1) mod 3 }
            property never_two { □ (x != 2) }
            ",
            5,
        );
        assert_eq!(report.status, CheckStatus::Fail);
        let trace = report.properties[0]
            .counterexample
            .as_ref()
            .expect("counterexample");
        assert!(trace
            .states
            .iter()
            .any(|s| matches!(s.values.first(), Some(caelum_kernel::model::Value::Int(2)))));

        // Safety holds within depth: □ (0 ≤ x ≤ 3) must PASS.
        let report = check_smtlib(
            r"
            let x: 0..3
            init { x = 0 }
            transition step { x' = (x + 1) mod 4 }
            property in_range { □ (x >= 0 ∧ x <= 3) }
            ",
            8,
        );
        assert_eq!(report.properties[0].status, CheckStatus::Pass);

        // Recurrence via lasso: □ ◇ (x = 0) must PASS.
        let report = check_smtlib(
            r"
            let x: 0..2
            init { x = 0 }
            transition step { x' = (x + 1) mod 3 }
            property recurrent_zero { □ ◇ (x = 0) }
            ",
            5,
        );
        assert_eq!(report.properties[0].status, CheckStatus::Pass);
    }
}
