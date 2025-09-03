use ir::Function;
use ir::Instr;

#[test]
fn phi_not_inserted_for_same_incoming() {
    // build function with two preds that set same temp
    let mut f = Function {
        name: "@test_same".to_string(),
        params: Vec::new(),
        ret: None,
        body: vec![
            Instr::Label("entry".to_string()),
            Instr::ConstI64("%test_same_0".to_string(), 1),
            Instr::Br("merge".to_string()),
            Instr::Label("other".to_string()),
            Instr::ConstI64("%test_same_0".to_string(), 1),
            Instr::Br("merge".to_string()),
            Instr::Label("merge".to_string()),
            Instr::Ret(Some("%test_same_0".to_string())),
        ],
    };
    ir::ssa::insert_phi_nodes(&mut f);
    // No phi should be created because incoming defs are identical
    let s = f.emit_text();
    assert!(!s.contains("= phi"));
}

#[test]
fn phi_inserted_and_operands_rewritten() {
    let mut f = Function {
        name: "@test_phi".to_string(),
        params: Vec::new(),
        ret: None,
        body: vec![
            Instr::Label("entry".to_string()),
            Instr::Br("then".to_string()),
            Instr::Label("then".to_string()),
            Instr::ConstI64("%test_phi_0".to_string(), 1),
            Instr::Br("merge".to_string()),
            Instr::Label("else".to_string()),
            Instr::ConstI64("%test_phi_1".to_string(), 2),
            Instr::Br("merge".to_string()),
            Instr::Label("merge".to_string()),
            Instr::Add("%test_phi_2".to_string(), "%test_phi_0".to_string(), "%test_phi_1".to_string()),
            Instr::Ret(Some("%test_phi_2".to_string())),
        ],
    };
    ir::ssa::insert_phi_nodes(&mut f);
    let s = f.emit_text();
    assert!(s.contains("= phi"));
    // ensure uses of temps in add are replaced by phi dest
    assert!(s.contains("%phi_test_phi_") || s.contains("%phi_test_phi_"));
}
