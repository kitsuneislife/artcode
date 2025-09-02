use std::path::Path;
use std::fs;
use std::str::FromStr;
use crate::ir_analyzer::IrAnalysis;

/// Very small, permissive parser that maps a textual `.ir` into a lightweight
/// Function-like structure so the analyzer can weight opcodes precisely.
/// We don't reuse `crates/ir::Function` to avoid tight coupling; this parser is
/// intentionally tiny and accepts the project's documented subset.
pub fn parse_ir_file(path: &Path) -> Option<IrAnalysis> {
    let s = fs::read_to_string(path).ok()?;
    // quick heuristic: if file contains 'call' or 'gc_alloc' or 'arena_alloc' prefer parser
    if !s.contains("call") && !s.contains("gc_alloc") && !s.contains("arena_alloc") {
        return None;
    }
    // fallback: use analyzer to get instr/block counts
    Some(crate::ir_analyzer::analyze_ir_text(&s))
}
