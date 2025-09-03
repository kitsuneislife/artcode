//! Simple IR textual analyzer used by the JIT tooling to estimate function cost.
//!
//! The analyzer implements a lightweight, permissive parser for the project's
//! textual IR. It counts instruction-like lines and basic blocks. The goal is
//! to provide a cheap cost metric for AOT/JIT heuristics without depending on
//! the full IR parser.

// Analyzer weight constants (can be tuned by calibrator)
pub const DEFAULT_WEIGHT: usize = 1;
pub const CALL_WEIGHT: usize = 5;
pub const ALLOC_WEIGHT: usize = 10;
pub const BLOCK_WEIGHT: usize = 2;

pub struct IrAnalysis {
    pub instr_count: usize, // raw instruction-like lines
    pub block_count: usize,
    pub call_count: usize,
    pub alloc_count: usize,
}

/// Analyze textual IR and return instruction and block counts.
///
/// Heuristics:
/// - Lines that end with ':' are considered block labels (e.g. `entry:`).
/// - Lines that are empty, comments (starting with `//`) or function headers
///   (`func @name(...) -> ... {`) and closing `}` are ignored.
/// - Remaining non-empty lines are considered instructions.
pub fn analyze_ir_text(s: &str) -> IrAnalysis {
    let mut instr = 0usize;
    let mut blocks = 0usize;
    let mut calls = 0usize;
    let mut allocs = 0usize;
    for line in s.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if t.starts_with("//") {
            continue;
        }
        // function header
        if t.starts_with("func @") && t.ends_with("{") {
            continue;
        }
        if t == "}" {
            continue;
        }
        if t.ends_with(":") {
            blocks += 1;
            continue;
        }
        // everything else is treated as an instruction-like line
        // weight certain opcodes higher because they indicate heavier work
        let lower = t.to_ascii_lowercase();
        if lower.starts_with("call ") || lower.contains(" call ") || lower.contains("= call") {
            instr += 1;
            calls += 1;
        } else if lower.contains("gc_alloc") || lower.contains("arena_alloc") {
            instr += 1;
            allocs += 1;
        } else {
            instr += 1;
        }
    }
    IrAnalysis {
        instr_count: instr,
        block_count: blocks,
        call_count: calls,
        alloc_count: allocs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_simple_function() {
        let text = r#"
func @add(i64 a, i64 b) -> i64 {
  entry:
    %0 = add i64 a, b
    ret %0
}
"#;
        let a = analyze_ir_text(text);
        assert_eq!(a.block_count, 1);
        assert_eq!(a.instr_count, 2);
    }

    #[test]
    fn ignores_comments_and_blank() {
        let text = r#"
func @f() -> void {
  entry:
    // this is a comment

    ret
}
"#;
        let a = analyze_ir_text(text);
        assert_eq!(a.block_count, 1);
        assert_eq!(a.instr_count, 1);
    }
}
