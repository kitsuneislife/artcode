use ir::ssa;
use ir::Function;
use ir::Instr;

#[test]
fn diamond_phi_insertion() {
  // build diamond CFG that requires a phi
  let mut f = Function {
    name: "@test_diamond".to_string(),
    params: Vec::new(),
    ret: None,
    body: vec![
      Instr::Label("entry".to_string()),
      Instr::Br("then".to_string()),
      Instr::Label("then".to_string()),
      Instr::ConstI64("%test_diamond_1".to_string(), 2),
      Instr::Br("merge".to_string()),
      Instr::Label("else".to_string()),
      Instr::ConstI64("%test_diamond_2".to_string(), 3),
      Instr::Br("merge".to_string()),
      Instr::Label("merge".to_string()),
      Instr::Add("%test_diamond_3".to_string(), "%test_diamond_1".to_string(), "%test_diamond_2".to_string()),
      Instr::Ret(Some("%test_diamond_3".to_string())),
    ],
  };

  ssa::insert_phi_nodes(&mut f);
  let s = f.emit_text();
  assert!(s.contains("= phi"), "expected phi in emitted IR: {}", s);
}
