//! General recursive lowering of a function body to IR.
//!
//! Covers the procedural AOT subset: `let` bindings, `if`/`else`, `while`,
//! `return`, and nested calls. Uses an alloca/load/store memory model so
//! `clang -O2` (mem2reg) promotes slots to registers in the output binary.
//!
//! `for` loops and collection iteration are outside the current AOT subset
//! (Artcode has no integer range syntax). Any unsupported construct causes
//! `lower_function` to return `None`, keeping the function out of AOT.

use crate::{CmpPred, Function, Instr, Type};
use core::ast::{ArtValue, Expr, FunctionParam, MatchPattern, Stmt};
use std::collections::HashMap;

struct Lowerer {
    fname: String,
    allocas: Vec<Instr>,
    body: Vec<Instr>,
    next_tmp: usize,
    next_lbl: usize,
    slots: HashMap<String, String>,
    terminated: bool,
}

impl Lowerer {
    fn new(fname: &str) -> Self {
        Lowerer {
            fname: fname.replace('@', ""),
            allocas: Vec::new(),
            body: Vec::new(),
            next_tmp: 0,
            next_lbl: 0,
            slots: HashMap::new(),
            terminated: false,
        }
    }

    fn tmp(&mut self) -> String {
        let t = format!("%t{}", self.next_tmp);
        self.next_tmp += 1;
        t
    }

    fn label(&mut self, kind: &str) -> String {
        let l = format!("{}_{}_{}", self.fname, kind, self.next_lbl);
        self.next_lbl += 1;
        l
    }

    fn slot_for(&mut self, name: &str) -> String {
        if let Some(s) = self.slots.get(name) {
            return s.clone();
        }
        let slot = format!("%{}.addr", name);
        self.allocas.push(Instr::Alloca(slot.clone()));
        self.slots.insert(name.to_string(), slot.clone());
        slot
    }

    fn push(&mut self, instr: Instr) {
        self.body.push(instr);
    }

    fn br_to(&mut self, label: &str) {
        if !self.terminated {
            self.push(Instr::Br(label.to_string()));
        }
    }

    fn start_block(&mut self, label: &str) {
        self.push(Instr::Label(label.to_string()));
        self.terminated = false;
    }

    fn lower_expr(&mut self, e: &Expr) -> Option<String> {
        match e {
            Expr::Literal(ArtValue::Int(n)) => Some(n.to_string()),
            Expr::Literal(ArtValue::Bool(b)) => Some(if *b { "1" } else { "0" }.to_string()),
            Expr::Variable { name } => {
                let slot = self.slots.get(&name.lexeme)?.clone();
                let dest = self.tmp();
                self.push(Instr::Load(dest.clone(), slot));
                Some(dest)
            }
            Expr::Grouping { expression } => self.lower_expr(expression),
            Expr::Unary { operator, right } if operator.lexeme == "-" => {
                let r = self.lower_expr(right)?;
                let dest = self.tmp();
                self.push(Instr::Sub(dest.clone(), "0".to_string(), r));
                Some(dest)
            }
            Expr::Binary {
                left,
                operator,
                right,
            } => {
                let l = self.lower_expr(left)?;
                let r = self.lower_expr(right)?;
                let dest = self.tmp();
                let op = operator.lexeme.as_str();
                let instr = match op {
                    "+" => Instr::Add(dest.clone(), l, r),
                    "-" => Instr::Sub(dest.clone(), l, r),
                    "*" => Instr::Mul(dest.clone(), l, r),
                    "/" => Instr::Div(dest.clone(), l, r),
                    "<" => Instr::ICmp(dest.clone(), CmpPred::Lt, l, r),
                    "<=" => Instr::ICmp(dest.clone(), CmpPred::Le, l, r),
                    ">" => Instr::ICmp(dest.clone(), CmpPred::Gt, l, r),
                    ">=" => Instr::ICmp(dest.clone(), CmpPred::Ge, l, r),
                    "==" => Instr::ICmp(dest.clone(), CmpPred::Eq, l, r),
                    "!=" => Instr::ICmp(dest.clone(), CmpPred::Ne, l, r),
                    _ => return None,
                };
                self.push(instr);
                Some(dest)
            }
            Expr::Call {
                callee,
                arguments,
                ..
            } => {
                let fname = match &**callee {
                    Expr::Variable { name } => name.lexeme.clone(),
                    _ => return None,
                };
                let mut args = Vec::new();
                for a in arguments {
                    args.push(self.lower_expr(a)?);
                }
                let dest = self.tmp();
                self.push(Instr::Call(dest.clone(), fname, args));
                Some(dest)
            }
            _ => None,
        }
    }

    fn lower_stmt(&mut self, s: &Stmt) -> Option<()> {
        if self.terminated {
            return Some(());
        }
        match s {
            Stmt::Block { statements } => {
                for st in statements {
                    self.lower_stmt(st)?;
                }
                Some(())
            }
            Stmt::Let {
                pattern,
                initializer,
                ..
            } => {
                let var_name = match pattern {
                    MatchPattern::Variable(tok) => tok.lexeme.clone(),
                    MatchPattern::Binding(tok) => tok.lexeme.clone(),
                    _ => return None,
                };
                let v = self.lower_expr(initializer)?;
                let slot = self.slot_for(&var_name);
                self.push(Instr::Store(slot, v));
                Some(())
            }
            Stmt::Expression(e) => {
                self.lower_expr(e)?;
                Some(())
            }
            Stmt::Return { value } => {
                let op = match value {
                    Some(e) => self.lower_expr(e)?,
                    None => "0".to_string(),
                };
                self.push(Instr::Ret(Some(op)));
                self.terminated = true;
                Some(())
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond = self.lower_expr(condition)?;
                let then_bb = self.label("then");
                let merge_bb = self.label("merge");
                let else_bb = if else_branch.is_some() {
                    self.label("else")
                } else {
                    merge_bb.clone()
                };
                self.push(Instr::BrCond(cond, then_bb.clone(), else_bb.clone()));
                self.terminated = true;

                self.start_block(&then_bb);
                self.lower_stmt(then_branch)?;
                self.br_to(&merge_bb);

                if let Some(eb) = else_branch {
                    self.start_block(&else_bb);
                    self.lower_stmt(eb)?;
                    self.br_to(&merge_bb);
                }

                self.start_block(&merge_bb);
                Some(())
            }
            Stmt::While { condition, body } => {
                let head_bb = self.label("while_head");
                let body_bb = self.label("while_body");
                let exit_bb = self.label("while_exit");
                self.br_to(&head_bb);

                self.start_block(&head_bb);
                let cond = self.lower_expr(condition)?;
                self.push(Instr::BrCond(cond, body_bb.clone(), exit_bb.clone()));
                self.terminated = true;

                self.start_block(&body_bb);
                self.lower_stmt(body)?;
                self.br_to(&head_bb);

                self.start_block(&exit_bb);
                Some(())
            }
            // For-loops iterate over collections — no integer range syntax in Artcode.
            // AOT lowering for collection iteration is not yet implemented.
            Stmt::For { .. } => None,
            _ => None,
        }
    }

    fn finish(mut self) -> Vec<Instr> {
        if !self.terminated {
            self.body.push(Instr::Ret(Some("0".to_string())));
        }
        let mut out = self.allocas;
        out.append(&mut self.body);
        out
    }
}

/// Lower a `Stmt::Function` to an IR `Function` using the general engine.
/// Returns `None` if the body contains any construct outside the AOT subset.
pub fn lower_function(stmt: &Stmt) -> Option<Function> {
    let Stmt::Function {
        name, params, body, ..
    } = stmt
    else {
        return None;
    };

    let func_name = name.lexeme.clone();
    let ir_params: Vec<(String, Type)> = params
        .iter()
        .map(|p: &FunctionParam| (p.name.lexeme.clone(), Type::I64))
        .collect();

    let mut lw = Lowerer::new(&func_name);
    // Materialise parameters into stack slots so they can be read uniformly.
    for (pname, _) in &ir_params {
        let slot = lw.slot_for(pname);
        lw.push(Instr::Store(slot, pname.clone()));
    }
    lw.lower_stmt(body)?;
    let body_instrs = lw.finish();

    Some(Function {
        name: func_name,
        params: ir_params,
        ret: Some(Type::I64),
        body: body_instrs,
    })
}
