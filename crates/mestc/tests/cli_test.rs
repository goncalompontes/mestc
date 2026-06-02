use std::path::PathBuf;
use std::process::Command;

fn mest_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mest"))
}

fn fixture_path(subdir: &str, name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests");
    p.push("fixtures");
    p.push(subdir);
    p.push(name);
    p
}

fn run_mest(args: &[&str]) -> (String, String, bool) {
    let output = Command::new(mest_bin())
        .args(args)
        .output()
        .expect("failed to run mest");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    print!("{stdout}");
    eprint!("{stderr}");
    (stdout, stderr, output.status.success())
}

// ── check command ──────────────────────────────────────────────────

#[test]
fn check_arithmetic() {
    let path = fixture_path("check", "arithmetic.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(ok, "check arithmetic failed\nstderr: {stderr}");
    assert!(stdout.contains("Int"), "stdout: {stdout:?}");
}

#[test]
fn check_bool_ops() {
    let path = fixture_path("check", "bool_ops.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(ok, "check bool_ops failed\nstderr: {stderr}");
    assert!(stdout.contains("Bool"), "stdout: {stdout:?}");
}

#[test]
fn check_if_then_else() {
    let path = fixture_path("check", "if_then_else.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(ok, "check if_then_else failed\nstderr: {stderr}");
    assert!(stdout.contains(": Int"), "stdout: {stdout:?}");
    assert!(stdout.contains("if true"), "stdout: {stdout:?}");
}

#[test]
fn check_fun_apply() {
    let path = fixture_path("check", "fun_apply.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(ok, "check fun_apply failed\nstderr: {stderr}");
    assert!(stdout.contains(": Int"), "stdout: {stdout:?}");
}

#[test]
fn check_let_binding() {
    let path = fixture_path("check", "let_binding.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(ok, "check let_binding failed\nstderr: {stderr}");
    assert!(stdout.contains(": Int"), "stdout: {stdout:?}");
    assert!(stdout.contains("let x"), "stdout: {stdout:?}");
}

#[test]
fn check_let_polymorphism() {
    let path = fixture_path("check", "let_polymorphism.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(ok, "check let_polymorphism failed\nstderr: {stderr}");
    assert!(stdout.contains(": Bool"), "stdout: {stdout:?}");
    assert!(stdout.contains("let id"), "stdout: {stdout:?}");
}

#[test]
fn check_match_expr() {
    let path = fixture_path("check", "match_expr.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(ok, "check match_expr failed\nstderr: {stderr}");
    assert!(stdout.contains(": Bool"), "stdout: {stdout:?}");
    assert!(stdout.contains("match 42"), "stdout: {stdout:?}");
}

#[test]
fn check_unary() {
    let path = fixture_path("check", "unary.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(ok, "check unary failed\nstderr: {stderr}");
    assert!(stdout.contains(": Bool"), "stdout: {stdout:?}");
}

// ── advanced check ────────────────────────────────────────────────

#[test]
fn check_nested_if() {
    let path = fixture_path("check", "nested_if.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(ok, "check nested_if failed\nstderr: {stderr}");
    assert!(stdout.contains(": Int"), "stdout: {stdout:?}");
}

#[test]
fn check_bool_chain() {
    let path = fixture_path("check", "bool_chain.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(ok, "check bool_chain failed\nstderr: {stderr}");
    assert!(stdout.contains(": Bool"), "stdout: {stdout:?}");
}

#[test]
fn check_let_add() {
    let path = fixture_path("check", "let_add.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(ok, "check let_add failed\nstderr: {stderr}");
    assert!(stdout.contains(": Int"), "stdout: {stdout:?}");
}

#[test]
fn check_nested_let() {
    let path = fixture_path("check", "nested_let.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(ok, "check nested_let failed\nstderr: {stderr}");
    assert!(stdout.contains(": Int"), "stdout: {stdout:?}");
}

#[test]
fn check_paren_expr() {
    let path = fixture_path("check", "paren_expr.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(ok, "check paren_expr failed\nstderr: {stderr}");
    assert!(stdout.contains(": Int"), "stdout: {stdout:?}");
}

#[test]
fn check_if_compare() {
    let path = fixture_path("check", "if_compare.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(ok, "check if_compare failed\nstderr: {stderr}");
    assert!(stdout.contains(": Int"), "stdout: {stdout:?}");
}

#[test]
fn check_match_if() {
    let path = fixture_path("check", "match_if.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(ok, "check match_if failed\nstderr: {stderr}");
    assert!(stdout.contains(": Bool"), "stdout: {stdout:?}");
}

#[test]
fn check_pow() {
    let path = fixture_path("check", "pow.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(ok, "check pow failed\nstderr: {stderr}");
    assert!(stdout.contains(": Int"), "stdout: {stdout:?}");
}

#[test]
fn check_neg() {
    let path = fixture_path("check", "neg.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(ok, "check neg failed\nstderr: {stderr}");
    assert!(stdout.contains(": Int"), "stdout: {stdout:?}");
}

#[test]
fn check_higher_order() {
    let path = fixture_path("check", "higher_order.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(ok, "check higher_order failed\nstderr: {stderr}");
    assert!(stdout.contains(": Int"), "stdout: {stdout:?}");
}

#[test]
fn check_let_not() {
    let path = fixture_path("check", "let_not.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(ok, "check let_not failed\nstderr: {stderr}");
    assert!(stdout.contains(": Bool"), "stdout: {stdout:?}");
}

#[test]
fn check_const_fn() {
    let path = fixture_path("check", "const_fn.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(ok, "check const_fn failed\nstderr: {stderr}");
    assert!(stdout.contains(": Int"), "stdout: {stdout:?}");
}

#[test]
fn check_match_wildcard() {
    let path = fixture_path("check", "match_wildcard.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(ok, "check match_wildcard failed\nstderr: {stderr}");
    assert!(stdout.contains(": Bool"), "stdout: {stdout:?}");
}

#[test]
fn check_precedence() {
    let path = fixture_path("check", "precedence.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(ok, "check precedence failed\nstderr: {stderr}");
    assert!(stdout.contains(": Int"), "stdout: {stdout:?}");
}

#[test]
fn check_id_id() {
    let path = fixture_path("check", "id_id.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(ok, "check id_id failed\nstderr: {stderr}");
    assert!(stdout.contains(": Int"), "stdout: {stdout:?}");
}

// ── check error cases ─────────────────────────────────────────────

#[test]
fn check_type_error() {
    let path = fixture_path("check", "type_error.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(!ok, "expected type error to fail, stdout: {stdout:?}");
    assert!(
        stderr.contains("type inference failed"),
        "stderr: {stderr:?}"
    );
}

#[test]
fn check_type_mismatch_binop() {
    let path = fixture_path("check", "type_mismatch_binop.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(!ok, "expected type error, stdout: {stdout:?}");
    assert!(stderr.contains("type inference failed"), "stderr: {stderr:?}");
}

#[test]
fn check_let_poly_type_error() {
    let path = fixture_path("check", "let_poly_type_error.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(!ok, "expected type error, stdout: {stdout:?}");
    assert!(stderr.contains("type inference failed"), "stderr: {stderr:?}");
}

#[test]
fn check_match_type_error() {
    let path = fixture_path("check", "match_type_error.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["check", path_s]);
    assert!(!ok, "expected type error, stdout: {stdout:?}");
    assert!(stderr.contains("type inference failed"), "stderr: {stderr:?}");
}

// ── run command ───────────────────────────────────────────────────

#[test]
fn run_simple() {
    let path = fixture_path("run", "simple.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["run", path_s]);
    assert!(ok, "run simple failed\nstderr: {stderr}");
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn run_arithmetic() {
    let path = fixture_path("run", "arithmetic.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["run", path_s]);
    assert!(ok, "run arithmetic failed\nstderr: {stderr}");
    assert_eq!(stdout.trim(), "7");
}

#[test]
fn run_bool_ops() {
    let path = fixture_path("run", "bool_ops.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["run", path_s]);
    assert!(ok, "run bool_ops failed\nstderr: {stderr}");
    assert_eq!(stdout.trim(), "true");
}

// ── advanced run ──────────────────────────────────────────────────

#[test]
fn run_nested_if() {
    let path = fixture_path("run", "nested_if.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["run", path_s]);
    assert!(ok, "run nested_if failed\nstderr: {stderr}");
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn run_let_add() {
    let path = fixture_path("run", "let_add.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["run", path_s]);
    assert!(ok, "run let_add failed\nstderr: {stderr}");
    assert_eq!(stdout.trim(), "3");
}

#[test]
fn run_let_chain() {
    let path = fixture_path("run", "let_chain.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["run", path_s]);
    assert!(ok, "run let_chain failed\nstderr: {stderr}");
    assert_eq!(stdout.trim(), "3");
}

#[test]
fn run_parens() {
    let path = fixture_path("run", "parens.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["run", path_s]);
    assert!(ok, "run parens failed\nstderr: {stderr}");
    assert_eq!(stdout.trim(), "10");
}

#[test]
fn run_comparison() {
    let path = fixture_path("run", "comparison.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["run", path_s]);
    assert!(ok, "run comparison failed\nstderr: {stderr}");
    assert_eq!(stdout.trim(), "true");
}

#[test]
fn run_apply() {
    let path = fixture_path("run", "apply.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["run", path_s]);
    assert!(ok, "run apply failed\nstderr: {stderr}");
    assert_eq!(stdout.trim(), "7");
}

#[test]
fn run_pow() {
    let path = fixture_path("run", "pow.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["run", path_s]);
    assert!(ok, "run pow failed\nstderr: {stderr}");
    assert_eq!(stdout.trim(), "25");
}

#[test]
fn run_if_compare() {
    let path = fixture_path("run", "if_compare_run.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["run", path_s]);
    assert!(ok, "run if_compare failed\nstderr: {stderr}");
    assert_eq!(stdout.trim(), "10");
}

// ── parse command ─────────────────────────────────────────────────

#[test]
fn parse_simple() {
    let path = fixture_path("parse", "simple.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["parse", path_s]);
    assert!(ok, "parse simple failed\nstderr: {stderr}");
    assert_eq!(stdout.trim(), "1 + 2");
}

// ── parse error cases ─────────────────────────────────────────────

#[test]
fn parse_error() {
    let path = fixture_path("parse", "parse_error.mest");
    let path_s = path.to_str().unwrap();
    let (stdout, stderr, ok) = run_mest(&["parse", path_s]);
    assert!(!ok, "expected parse error, stdout: {stdout:?}");
    assert!(stderr.contains("Error"), "stderr: {stderr:?}");
}

// ── eval command ──────────────────────────────────────────────────

#[test]
fn eval_inline() {
    let (stdout, stderr, ok) = run_mest(&["eval", "1 + 2"]);
    assert!(ok, "eval failed\nstderr: {stderr}");
    assert_eq!(stdout.trim(), "3");
}

// ── lex command ───────────────────────────────────────────────────

#[test]
fn lex_inline() {
    let (stdout, stderr, ok) = run_mest(&["lex", "1 + 2"]);
    assert!(ok, "lex failed\nstderr: {stderr}");
    assert!(stdout.contains("Int(1)"), "stdout: {stdout:?}");
    assert!(stdout.contains("Plus"), "stdout: {stdout:?}");
    assert!(stdout.contains("Int(2)"), "stdout: {stdout:?}");
}
