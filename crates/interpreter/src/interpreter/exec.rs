use super::actors::{ActorState, Mailbox};
use super::Interpreter;
use crate::values::{Result, RuntimeError};
use core::ast::{ArtValue, Function, MatchPattern, Stmt};
use core::Token;
use core::environment::Environment;
use diagnostics::{Diagnostic, DiagnosticKind, Span};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

impl Interpreter {
    pub(super) fn bind_value_to_pattern(
        &mut self,
        pattern: &core::ast::MatchPattern,
        value: ArtValue,
    ) -> Result<()> {
        match pattern {
            core::ast::MatchPattern::Variable(name) => {
                // Runtime check: evitar que valores alocados em arena escapem para fora do bloco performant.
                if let ArtValue::HeapComposite(h) = &value
                    && let Some(obj) = self.heap_objects.get(&h.0)
                    && let Some(aid) = obj.arena_id
                    && Some(aid) != self.current_arena
                {
                    let msg = format!(
                        "Attempt to bind arena object (arena={}) into scope outside of that arena (current_arena={:?}) for variable '{}'.",
                        aid, self.current_arena, name.lexeme
                    );
                    debug_assert!(!msg.is_empty(), "{}", msg);
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        msg,
                        Span::new(name.start, name.end, name.line, name.col),
                    ));
                }

                // Captura possível valor antigo sem manter borrow mutável durante decremento
                let old_opt = {
                    self.environment
                        .borrow()
                        .values
                        .get(name.lexeme.as_str())
                        .cloned()
                };
                if let Some(old) = &old_opt {
                    self.dec_value_if_heap(old);
                }
                let mut env = self.environment.borrow_mut();
                env.define(&name.lexeme, value);
                Ok(())
            }
            core::ast::MatchPattern::Tuple(patterns) => {
                if let ArtValue::Tuple(values) = value {
                    if patterns.len() != values.len() {
                        return Err(RuntimeError::TypeError(format!(
                            "Tuple pattern length {} does not match tuple value length {}",
                            patterns.len(),
                            values.len()
                        )));
                    }
                    for (p, v) in patterns.iter().zip(values.into_iter()) {
                        self.bind_value_to_pattern(p, v)?;
                    }
                    Ok(())
                } else {
                    return Err(RuntimeError::TypeError(format!(
                        "Cannot destructure non-tuple value '{:?}' with tuple pattern",
                        value
                    )));
                }
            }
            _ => {
                // Ignore other patterns for `let` declarations for now (or throw error if unsupported)
                Ok(())
            }
        }
    }

    fn debug_prompt(&mut self, stmt: &core::ast::Stmt) -> Result<bool> {
        use std::io::{self, Write};
        loop {
            println!("\n[Tick {}] {:?}", self.executed_statements, stmt);
            print!("(art-debug) > ");
            let _ = io::stdout().flush();

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                break;
            }
            let input = input.trim();

            match input {
                "" | "step" | "s" => return Ok(false),
                "back" | "b" => return Ok(true),
                "env" => {
                    println!("Environment bindings:");
                    for (k, _) in self.environment.borrow().values.iter() {
                        println!(" - {}", k);
                    }
                }
                "help" => {
                    println!("Commands:");
                    println!("  step (s)      - Avança 1 statement (Default)");
                    println!(
                        "  back (b)      - Volta 1 statement no tempo via snapshotting rápido"
                    );
                    println!(
                        "  inspect <var> - Avalia nome da variável no contexto local ou global"
                    );
                    println!("  env           - Lista escopo");
                    println!("  help          - Mostra essa ajuda");
                }
                other if other.starts_with("inspect ") => {
                    let var = &other[8..];
                    if let Some(val) = self.get_global(var) {
                        println!("{} = {:?}", var, val);
                    } else {
                        println!("Variable '{}' not found.", var);
                    }
                }
                _ => println!("Unknown command. Type 'help'."),
            }
        }
        Ok(false)
    }

    pub(super) fn execute(&mut self, stmt: Stmt) -> Result<()> {
        if self.debug_mode || self.fast_forward_until.is_some() {
            let should_prompt = match self.fast_forward_until {
                Some(target) => self.executed_statements >= target,
                None => self.debug_mode,
            };
            if should_prompt {
                self.fast_forward_until = None;
                self.debug_mode = true;
                if self.debug_prompt(&stmt)? {
                    return Err(crate::values::RuntimeError::DebugStepBack);
                }
            }
        }
        self.executed_statements += 1;
        let result = match stmt {
            Stmt::Expression(expr) => {
                let val = self.evaluate(expr)?;
                self.last_value = Some(val.clone());
                Ok(())
            }
            Stmt::Let {
                pattern,
                ty: _,
                initializer,
            } => {
                let value = self.evaluate(initializer)?;

                // CRITICAL: If the actor was parked during evaluation (e.g. by actor_receive),
                // do NOT bind the pattern yet. The statement will be retried when unparked.
                // Reinsertion is handled by the round-robin scheduler (run_actors_round_robin).
                if self
                    .executing_actor
                    .as_ref()
                    .map(|a| a.parked)
                    .unwrap_or(false)
                {
                    return Ok(());
                }

                // Runtime check: evitar que valores alocados em arena escapem para fora do bloco performant.
                if let ArtValue::HeapComposite(h) = &value
                    && let Some(obj) = self.heap_objects.get(&h.0)
                    && let Some(aid) = obj.arena_id
                    && (self.in_performant_block || Some(aid) != self.current_arena)
                {
                    let msg = format!(
                        "Attempt to bind arena object (arena={}) into scope outside of that arena (current_arena={:?}) for variable '{}'.",
                        aid,
                        self.current_arena,
                        match &pattern {
                            core::ast::MatchPattern::Variable(name) => name.lexeme.clone(),
                            _ => "<pattern>".to_string(),
                        }
                    );
                    debug_assert!(!msg.is_empty(), "{}", msg);
                    let fn_name = if let core::ast::MatchPattern::Variable(name) = &pattern {
                        name
                    } else {
                        &core::Token::dummy("_")
                    };
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        msg,
                        Span::new(fn_name.start, fn_name.end, fn_name.line, fn_name.col),
                    ));
                }

                // Promoção se necessário
                let aid = self.environment.borrow().associated_arena;
                let mut promoted_value = value;
                self.promote_if_escaping(aid, &mut promoted_value);

                self.bind_value_to_pattern(&pattern, promoted_value)?;
                Ok(())
            }
            Stmt::Block { statements } => {
                self.execute_block(statements, Some(self.environment.clone()))
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition_value = self.evaluate(condition)?;
                if self.is_truthy(&condition_value) {
                    self.execute(*then_branch)
                } else if let Some(else_stmt) = else_branch {
                    self.execute(*else_stmt)
                } else {
                    Ok(())
                }
            }
            Stmt::IfLet {
                pattern,
                value,
                then_branch,
                else_branch,
            } => {
                let eval_value = self.evaluate(value)?;
                if let Some(bindings) = self.pattern_matches(&pattern, &eval_value) {
                    let (parent_depth, parent_arena) = {
                        let b = self.environment.borrow();
                        (b.depth, b.associated_arena)
                    };
                    let mut new_env = Environment::new(
                        Some(self.environment.clone()),
                        parent_depth + 1,
                        parent_arena,
                    );
                    for (k, mut v) in bindings {
                        let target_aid = new_env.associated_arena;
                        self.promote_if_escaping(target_aid, &mut v);
                        new_env.define(&k, v);
                    }
                    let previous = self.environment.clone();
                    self.environment = Rc::new(RefCell::new(new_env));
                    let res = self.execute(*then_branch);
                    self.environment = previous;
                    res
                } else if let Some(else_stmt) = else_branch {
                    self.execute(*else_stmt)
                } else {
                    Ok(())
                }
            }
            Stmt::StructDecl { name, fields } => {
                self.type_registry.register_struct(name, fields);
                Ok(())
            }
            Stmt::EnumDecl { name, variants } => {
                self.type_registry.register_enum(name, variants);
                Ok(())
            }
            Stmt::Match { expr, cases } => {
                let match_value = self.evaluate(expr)?;
                for (pattern, guard, stmt) in cases {
                    if let Some(bindings) = self.pattern_matches(&pattern, &match_value) {
                        let (p_depth, p_arena) = {
                            let b = self.environment.borrow();
                            (b.depth, b.associated_arena)
                        };
                        // Avaliar guard (se existir) em ambiente com bindings temporário
                        if let Some(gexpr) = guard {
                            let previous_env = self.environment.clone();
                            let temp_env = Rc::new(RefCell::new(Environment::new(
                                Some(previous_env.clone()),
                                p_depth + 1,
                                p_arena,
                            )));
                            self.environment = temp_env.clone();
                            for (name, value) in bindings.iter() {
                                self.environment.borrow_mut().define(name, value.clone());
                            }
                            let guard_passed = self
                                .evaluate(gexpr)
                                .map(|v| self.is_truthy(&v))
                                .unwrap_or(false);
                            // Garantir que handles fortes do ambiente temporário do guard sejam decrementados
                            self.drop_scope_heap_objects(&temp_env);
                            self.environment = previous_env;
                            if !guard_passed {
                                continue;
                            }
                        }
                        let (p_depth, p_arena) = {
                            let b = self.environment.borrow();
                            (b.depth, b.associated_arena)
                        };
                        let new_env_struct =
                            Environment::new(Some(self.environment.clone()), p_depth + 1, p_arena);
                        let new_env = Rc::new(RefCell::new(new_env_struct));
                        let previous = self.environment.clone();
                        self.environment = new_env.clone();
                        for (name, mut value) in bindings {
                            let target_aid = new_env.borrow().associated_arena;
                            self.promote_if_escaping(target_aid, &mut value);
                            new_env.borrow_mut().define(&name, value);
                        }
                        // Executar o corpo e garantir que mesmo em erro o escopo temporário seja limpo
                        let result = self.execute(stmt);
                        // Drop handles do env de bindings antes de restaurar
                        self.drop_scope_heap_objects(&new_env);
                        self.environment = previous;
                        return result;
                    }
                }

                // Se chegou aqui, nenhum pattern casou (Non-exhaustive pattern match no Runtime)
                self.diagnostics.push(Diagnostic::new(
                    DiagnosticKind::Runtime,
                    format!(
                        "Non-exhaustive match: no pattern matched the value '{:?}'",
                        match_value
                    ),
                    Span::new(0, 0, 0, 0), // Idealmente teríamos o span do Stmt::Match
                ));
                Ok(())
            }
            Stmt::TryCatch {
                try_branch,
                catch_name,
                catch_branch,
            } => match self.execute(*try_branch) {
                Ok(()) => Ok(()),
                Err(RuntimeError::Return(v)) => Err(RuntimeError::Return(v)),
                Err(RuntimeError::DebugStepBack) => Err(RuntimeError::DebugStepBack),
                Err(RuntimeError::TypeError(msg)) => {
                    let previous_env = self.environment.clone();
                    let (p_depth, p_arena) = {
                        let b = previous_env.borrow();
                        (b.depth, b.associated_arena)
                    };
                    let catch_env = Rc::new(RefCell::new(Environment::new(
                        Some(previous_env.clone()),
                        p_depth + 1,
                        p_arena,
                    )));
                    self.environment = catch_env.clone();

                    self.environment
                        .borrow_mut()
                        .define(&catch_name.lexeme, ArtValue::String(Arc::from(msg)));

                    let result = self.execute(*catch_branch);

                    self.drop_scope_heap_objects(&catch_env);
                    self.environment = previous_env;

                    result
                }
            },
            Stmt::Function {
                name,
                type_params,
                params,
                return_type: _,
                body,
                method_owner,
                is_async: _,
            } => {
                let fn_rc = Rc::new(Function {
                    name: Some(name.lexeme.clone()),
                    type_params: type_params.clone(),
                    params: params.clone(),
                    body: body.clone(),
                    closure: Rc::downgrade(&self.environment),
                    retained_env: None,
                });
                if let Some(owner) = method_owner {
                    if let Some(sdef) = self.type_registry.structs.get_mut(&owner) {
                        sdef.methods.insert(name.lexeme.clone(), (*fn_rc).clone());
                    } else if let Some(edef) = self.type_registry.enums.get_mut(&owner) {
                        edef.methods.insert(name.lexeme.clone(), (*fn_rc).clone());
                    } else {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("Unknown type '{}' for method.", owner),
                            Span::new(name.start, name.end, name.line, name.col),
                        ));
                    }
                } else {
                    let old_opt = {
                        self.environment
                            .borrow()
                            .values
                            .get(name.lexeme.as_str())
                            .cloned()
                    };
                    if let Some(old) = &old_opt {
                        self.dec_value_if_heap(old);
                    }
                    let mut env = self.environment.borrow_mut();
                    env.define(&name.lexeme, ArtValue::Function(fn_rc.clone()));
                }
                Ok(())
            }
            Stmt::ImplBlock { methods, .. } => {
                for method in methods {
                    self.execute(method)?;
                }
                Ok(())
            }
            Stmt::Return { value } => {
                let return_value = match value {
                    Some(expr) => self.evaluate(expr)?,
                    None => ArtValue::none(),
                };
                // Runtime check: impedir retorno de objetos de arena para fora do bloco performant
                if let ArtValue::HeapComposite(h) = &return_value
                    && let Some(obj) = self.heap_objects.get(&h.0)
                    && let Some(aid) = obj.arena_id
                    && (self.in_performant_block || Some(aid) != self.current_arena)
                {
                    let msg = format!(
                        "Attempt to return arena object (arena={}) outside of its arena (current_arena={:?}).",
                        aid, self.current_arena
                    );
                    debug_assert!(!msg.is_empty(), "{}", msg);
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        msg,
                        Span::new(0, 0, 0, 0),
                    ));
                }
                Err(RuntimeError::Return(return_value))
            }
            Stmt::Performant { statements } => {
                // Criar arena ID e frame léxico
                let aid = self.next_arena_id;
                self.next_arena_id += 1;
                let prev_arena = self.current_arena;
                self.current_arena = Some(aid);
                let prev_performant = self.in_performant_block;
                self.in_performant_block = true;

                let previous = self.environment.clone();
                let (p_depth, _) = {
                    let b = previous.borrow();
                    (b.depth, b.associated_arena)
                };

                self.environment = Rc::new(RefCell::new(Environment::new(
                    Some(previous.clone()),
                    p_depth + 1,
                    Some(aid),
                )));
                let scope_env = self.environment.clone();

                // Executar statements
                for s in statements {
                    if let Err(e) = self.execute(s) {
                        self.drop_scope_heap_objects(&scope_env);
                        self.finalize_arena(aid);
                        self.current_arena = prev_arena;
                        self.in_performant_block = prev_performant;
                        self.environment = previous;
                        return Err(e);
                    }
                }

                // Cleanup
                self.drop_scope_heap_objects(&scope_env);
                self.finalize_arena(aid);
                self.current_arena = prev_arena;
                self.in_performant_block = prev_performant;
                self.environment = previous;
                Ok(())
            }
            Stmt::Import { path: _ } => {
                // Import is a compile-time / resolver concern; runtime no-op for now.
                Ok(())
            }
            Stmt::ShellCommand { program, args } => {
                if !self.ensure_pure_allowed("shell") {
                    let blocked = Self::shell_result_err(
                        "Operation 'shell' is not allowed in --pure mode".to_string(),
                    );
                    self.publish_shell_result(blocked);
                    return Ok(());
                }

                match self.run_shell_stages(&program, &args) {
                    Ok(output) => {
                        if !output.stdout.is_empty() {
                            print!("{}", String::from_utf8_lossy(&output.stdout));
                        }
                        if !output.stderr.is_empty() {
                            eprint!("{}", String::from_utf8_lossy(&output.stderr));
                        }
                        let result = if output.status.success() {
                            Self::shell_result_ok(
                                String::from_utf8_lossy(&output.stdout).to_string(),
                            )
                        } else if output.stderr.is_empty() {
                            Self::shell_result_err(format!(
                                "Shell command '{}' exited with status {:?}",
                                program,
                                output.status.code()
                            ))
                        } else {
                            Self::shell_result_err(
                                String::from_utf8_lossy(&output.stderr).to_string(),
                            )
                        };
                        self.publish_shell_result(result);
                        Ok(())
                    }
                    Err(e) => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            e.clone(),
                            Span::new(0, 0, 0, 0),
                        ));
                        self.publish_shell_result(Self::shell_result_err(e));
                        Ok(())
                    }
                }
            }
            Stmt::While { condition, body } => {
                loop {
                    let cond_val = self.evaluate(condition.clone())?;
                    if !self.is_truthy(&cond_val) {
                        break;
                    }
                    let _aid = self.push_implicit_arena();
                    let res = self.execute(*body.clone());
                    self.pop_implicit_arena();
                    if let Err(e) = res {
                        return Err(e);
                    }
                }
                Ok(())
            }
            Stmt::For {
                element,
                iterator,
                body,
            } => {
                let iter_val = self.evaluate(iterator)?;

                // Support: arrays, stream pipelines, or iterator protocols (callable returning Option).
                // This allows generators to be implemented as closures returning Option.None.
                enum IterSource {
                    Array(Vec<ArtValue>),
                    Iterator,
                }

                let iter_source = match iter_val.clone() {
                    ArtValue::Array(arr) => IterSource::Array(arr),
                    ArtValue::StructInstance {
                        ref struct_name, ..
                    } if struct_name == "__Stream" => {
                        match self.decode_stream_value(iter_val.clone()) {
                            Ok((source, ops)) => {
                                IterSource::Array(self.run_stream_pipeline(source, ops)?)
                            }
                            Err(msg) => {
                                self.diagnostics.push(Diagnostic::new(
                                    DiagnosticKind::Runtime,
                                    msg,
                                    Span::new(
                                        element.start,
                                        element.end,
                                        element.line,
                                        element.col,
                                    ),
                                ));
                                return Ok(());
                            }
                        }
                    }
                    ArtValue::HeapComposite(h) => {
                        match self.heap_objects.get(&h.0).map(|obj| obj.value.clone()) {
                            Some(ArtValue::Array(arr)) => IterSource::Array(arr),
                            Some(
                                ref v @ ArtValue::StructInstance {
                                    ref struct_name, ..
                                },
                            ) if struct_name == "__Stream" => {
                                match self.decode_stream_value(v.clone()) {
                                    Ok((source, ops)) => {
                                        IterSource::Array(self.run_stream_pipeline(source, ops)?)
                                    }
                                    Err(msg) => {
                                        self.diagnostics.push(Diagnostic::new(
                                            DiagnosticKind::Runtime,
                                            msg,
                                            Span::new(
                                                element.start,
                                                element.end,
                                                element.line,
                                                element.col,
                                            ),
                                        ));
                                        return Ok(());
                                    }
                                }
                            }
                            Some(_) => IterSource::Iterator,
                            None => {
                                self.diagnostics.push(Diagnostic::new(
                                    DiagnosticKind::Runtime,
                                    "Cannot iterate over dangling heap handle.".to_string(),
                                    Span::new(
                                        element.start,
                                        element.end,
                                        element.line,
                                        element.col,
                                    ),
                                ));
                                return Ok(());
                            }
                        }
                    }
                    ArtValue::Function(_) | ArtValue::Builtin(_) => IterSource::Iterator,
                    _ => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("Cannot iterate over unsupported type: {:?}", iter_val),
                            Span::new(element.start, element.end, element.line, element.col),
                        ));
                        return Ok(());
                    }
                };

                // Helper to call iterator `next()` and return the next value (or None).
                let call_next = |me: &mut Interpreter, iter_val: &ArtValue| -> Result<ArtValue> {
                    match iter_val {
                        ArtValue::Function(func) => {
                            me.call_function(func.clone(), None, Vec::new())
                        }
                        ArtValue::Builtin(b) => me.call_builtin(b.clone(), Vec::new()),
                        ArtValue::StructInstance {
                            struct_name,
                            fields,
                        } => {
                            let token = Token::dummy("next");
                            if let Some(callable) = crate::field_access::struct_field_or_method(
                                struct_name,
                                fields,
                                &token,
                                &me.type_registry,
                            ) {
                                match callable {
                                    ArtValue::Function(func) => {
                                        me.call_function(func, None, Vec::new())
                                    }
                                    ArtValue::Builtin(b) => me.call_builtin(b, Vec::new()),
                                    other => {
                                        me.diagnostics.push(Diagnostic::new(
                                            DiagnosticKind::Runtime,
                                            format!(
                                                "Iterator 'next' must be callable, got: {:?}",
                                                other
                                            ),
                                            Span::new(
                                                element.start,
                                                element.end,
                                                element.line,
                                                element.col,
                                            ),
                                        ));
                                        Ok(ArtValue::none())
                                    }
                                }
                            } else {
                                me.diagnostics.push(Diagnostic::new(
                                    DiagnosticKind::Runtime,
                                    "Iterator object does not implement 'next()' method."
                                        .to_string(),
                                    Span::new(
                                        element.start,
                                        element.end,
                                        element.line,
                                        element.col,
                                    ),
                                ));
                                Ok(ArtValue::none())
                            }
                        }
                        ArtValue::HeapComposite(_) => {
                            let resolved = me.resolve_composite(iter_val).clone();
                            if let ArtValue::StructInstance {
                                struct_name,
                                fields,
                            } = resolved
                            {
                                let token = Token::dummy("next");
                                if let Some(callable) = crate::field_access::struct_field_or_method(
                                    &struct_name,
                                    &fields,
                                    &token,
                                    &me.type_registry,
                                ) {
                                    match callable {
                                        ArtValue::Function(func) => {
                                            me.call_function(func, None, Vec::new())
                                        }
                                        ArtValue::Builtin(b) => me.call_builtin(b, Vec::new()),
                                        other => {
                                            me.diagnostics.push(Diagnostic::new(
                                                DiagnosticKind::Runtime,
                                                format!(
                                                    "Iterator 'next' must be callable, got: {:?}",
                                                    other
                                                ),
                                                Span::new(
                                                    element.start,
                                                    element.end,
                                                    element.line,
                                                    element.col,
                                                ),
                                            ));
                                            Ok(ArtValue::none())
                                        }
                                    }
                                } else {
                                    me.diagnostics.push(Diagnostic::new(
                                        DiagnosticKind::Runtime,
                                        "Iterator object does not implement 'next()' method."
                                            .to_string(),
                                        Span::new(
                                            element.start,
                                            element.end,
                                            element.line,
                                            element.col,
                                        ),
                                    ));
                                    Ok(ArtValue::none())
                                }
                            } else {
                                me.diagnostics.push(Diagnostic::new(
                                    DiagnosticKind::Runtime,
                                    format!("Cannot iterate over unsupported type: {:?}", iter_val),
                                    Span::new(
                                        element.start,
                                        element.end,
                                        element.line,
                                        element.col,
                                    ),
                                ));
                                Ok(ArtValue::none())
                            }
                        }
                        _ => {
                            me.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                format!("Cannot iterate over unsupported type: {:?}", iter_val),
                                Span::new(element.start, element.end, element.line, element.col),
                            ));
                            Ok(ArtValue::none())
                        }
                    }
                };

                match iter_source {
                    IterSource::Array(array_elements) => {
                        for mut val in array_elements {
                            let previous_env = self.environment.clone();
                            let (p_depth, p_arena) = {
                                let b = previous_env.borrow();
                                (b.depth, b.associated_arena)
                            };
                            let loop_env = Rc::new(RefCell::new(Environment::new(
                                Some(previous_env.clone()),
                                p_depth + 1,
                                p_arena,
                            )));
                            self.environment = loop_env.clone();

                            let target_aid = loop_env.borrow().associated_arena;
                            self.promote_if_escaping(target_aid, &mut val);
                            self.environment.borrow_mut().define(&element.lexeme, val);

                            let result = self.execute(*body.clone());

                            self.drop_scope_heap_objects(&loop_env);
                            self.environment = previous_env;

                            if let Err(e) = result {
                                return Err(e);
                            }
                        }
                        Ok(())
                    }
                    IterSource::Iterator => {
                        let iter_val = iter_val;
                        loop {
                            let next_val = call_next(self, &iter_val)?;
                            let next_val = self.resolve_composite(&next_val).clone();
                            let mut item = match next_val {
                                ArtValue::Optional(boxed) => match *boxed {
                                    Some(v) => v,
                                    None => break,
                                },
                                ArtValue::EnumInstance {
                                    enum_name,
                                    variant,
                                    values,
                                } if enum_name == "Option" => {
                                    if variant == "Some" {
                                        values.into_iter().next().unwrap_or(ArtValue::none())
                                    } else {
                                        break;
                                    }
                                }
                                other => {
                                    self.diagnostics.push(Diagnostic::new(
                                        DiagnosticKind::Runtime,
                                        format!(
                                            "Iterator protocol expected Optional, got: {:?}",
                                            other
                                        ),
                                        Span::new(
                                            element.start,
                                            element.end,
                                            element.line,
                                            element.col,
                                        ),
                                    ));
                                    break;
                                }
                            };

                            let previous_env = self.environment.clone();
                            let (p_depth, _p_arena) = {
                                let b = previous_env.borrow();
                                (b.depth, b.associated_arena)
                            };
                            // Iniciamos uma arena para o corpo do loop
                            let _aid = self.push_implicit_arena();
                            let loop_env = Rc::new(RefCell::new(Environment::new(
                                Some(previous_env.clone()),
                                p_depth + 1,
                                Some(_aid),
                            )));
                            self.environment = loop_env.clone();

                            let target_aid = loop_env.borrow().associated_arena;
                            self.promote_if_escaping(target_aid, &mut item);
                            self.environment.borrow_mut().define(&element.lexeme, item);

                            let result = self.execute(*body.clone());

                            self.drop_scope_heap_objects(&loop_env);
                            self.pop_implicit_arena();
                            self.environment = previous_env;

                            if let Err(e) = result {
                                return Err(e);
                            }
                        }
                        Ok(())
                    }
                }
            }
            Stmt::SpawnActor { body } => {
                let aid = self.next_actor_id;
                self.next_actor_id += 1;
                let actor_env = Rc::new(RefCell::new(Environment::new(
                    Some(self.environment.clone()),
                    0,    // Atores começam novo root de depth
                    None, // e usam ARC global por padrão (Nexus isolation)
                )));
                let actor = ActorState {
                    id: aid,
                    mailbox: Mailbox::new(),
                    body: VecDeque::from(body),
                    env: actor_env,
                    finished: false,
                    parked: false,
                    mailbox_limit: self.actor_mailbox_limit,
                };
                self.actors.insert(aid, actor);
                // Return actor handle as Actor variant (IDs still exposed as Int in tests where needed)
                self.last_value = Some(ArtValue::Actor(aid));
                Ok(())
            }
        };

        if result.is_ok() {
            self.maybe_record_checkpoint();
        }

        result
    }

    pub(super) fn pattern_matches(
        &mut self,
        pattern: &MatchPattern,
        value: &ArtValue,
    ) -> Option<Vec<(String, ArtValue)>> {
        // Se valor for HeapComposite, desreferencia para o valor real subjacente antes de matching.
        let resolved_owned;
        let value_ref = if let ArtValue::HeapComposite(h) = value {
            if let Some(obj) = self.heap_objects.get(&h.0) {
                resolved_owned = obj.value.clone();
                &resolved_owned
            } else {
                value
            }
        } else {
            value
        };
        match (pattern, value_ref) {
            (MatchPattern::Literal(lit), _) if lit == value => Some(vec![]),
            (MatchPattern::Wildcard, _) => Some(vec![]),
            // Se o binding está dentro de EnumVariant, associe ao valor correto
            (MatchPattern::Binding(name) | MatchPattern::Variable(name), val) => {
                // Se val for EnumInstance com um valor, associe ao primeiro valor
                if let ArtValue::EnumInstance { values, .. } = val {
                    if values.len() == 1 {
                        Some(vec![(name.lexeme.clone(), values[0].clone())])
                    } else {
                        // Se não, associe ao próprio valor
                        Some(vec![(name.lexeme.clone(), val.clone())])
                    }
                } else {
                    Some(vec![(name.lexeme.clone(), val.clone())])
                }
            }
            (
                MatchPattern::EnumVariant {
                    enum_name,
                    variant,
                    params,
                },
                ArtValue::EnumInstance {
                    enum_name: inst_enum_name,
                    variant: v_name,
                    values,
                    ..
                },
            ) if &variant.lexeme == v_name => {
                // Verificar nome do enum se especificado
                if let Some(enum_name_tok) = enum_name
                    && &enum_name_tok.lexeme != inst_enum_name
                {
                    return None;
                }
                match params {
                    Some(param_patterns) => {
                        if param_patterns.len() != values.len() {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                format!(
                                    "Arity mismatch in pattern: expected {} found {}",
                                    values.len(),
                                    param_patterns.len()
                                ),
                                Span::new(variant.start, variant.end, variant.line, variant.col),
                            ));
                            return None;
                        }
                        let mut all_bindings = Vec::new();
                        for (i, p) in param_patterns.iter().enumerate() {
                            if let Some(bindings) = self.pattern_matches(p, &values[i]) {
                                all_bindings.extend(bindings);
                            } else {
                                return None;
                            }
                        }
                        Some(all_bindings)
                    }
                    None => {
                        if values.is_empty() {
                            Some(vec![])
                        } else {
                            None
                        }
                    }
                }
            }
            _ => None,
        }
    }

    pub(super) fn execute_block(
        &mut self,
        statements: Vec<Stmt>,
        enclosing: Option<Rc<RefCell<Environment>>>,
    ) -> Result<()> {
        let (p_depth, p_arena) = if let Some(ref e) = enclosing {
            let b = e.borrow();
            (b.depth, b.associated_arena)
        } else {
            (0, None)
        };
        let previous = self.environment.clone();
        self.environment = Rc::new(RefCell::new(Environment::new(
            enclosing,
            p_depth + 1,
            p_arena,
        )));
        let scope_env = self.environment.clone();
        for statement in statements {
            if self
                .executing_actor
                .as_ref()
                .map(|a| a.parked)
                .unwrap_or(false)
            {
                break;
            }
            if let Err(e) = self.execute(statement) {
                // If this is a Return carrying values that depend on the current scope,
                // preserve them BEFORE dropping the block environment.
                let transformed = match e {
                    RuntimeError::Return(mut rv) => {
                        // Promoção Adaptativa ao retornar valores
                        let target_aid = previous.borrow().associated_arena;
                        self.promote_if_escaping(target_aid, &mut rv);
                        if let ArtValue::HeapComposite(ref h) = rv {
                            // Keep heap composite alive across scope teardown.
                            self.inc_heap_strong(h.0);
                        }
                        if let ArtValue::Function(ref f) = rv {
                            if f.retained_env.is_none() {
                                // Returned closures can capture this block env.
                                let escaped = Function {
                                    name: f.name.clone(),
                                    type_params: f.type_params.clone(),
                                    params: f.params.clone(),
                                    body: f.body.clone(),
                                    closure: f.closure.clone(),
                                    retained_env: Some(scope_env.clone()),
                                };
                                let mut rv = ArtValue::Function(Rc::new(escaped));
                                let aid = previous.borrow().associated_arena;
                                self.promote_if_escaping(aid, &mut rv);
                                return Err(RuntimeError::Return(rv));
                            }
                        }
                        RuntimeError::Return(rv)
                    }
                    other => other,
                };
                self.drop_scope_heap_objects(&scope_env);
                self.environment = previous;
                return Err(transformed);
            }
        }
        self.drop_scope_heap_objects(&scope_env);
        self.environment = previous;
        Ok(())
    }

}
