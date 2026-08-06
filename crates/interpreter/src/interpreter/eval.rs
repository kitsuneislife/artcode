use super::Interpreter;
use super::actors::{ActorState, Mailbox};
use super::{PRELUDE_VALUES, did_you_mean};
use crate::values::{Result, RuntimeError};
use core::Token;
use core::ast::{ArtValue, Expr, Function};
use core::environment::Environment;
use diagnostics::{Diagnostic, DiagnosticKind, Span};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

impl Interpreter {
    pub(super) fn evaluate(&mut self, expr: Expr) -> Result<ArtValue> {
        const MAX_EVAL_DEPTH: usize = 128;
        self.eval_depth += 1;
        if self.eval_depth > MAX_EVAL_DEPTH {
            self.eval_depth -= 1;
            self.diagnostics.push(Diagnostic::new(
                DiagnosticKind::Runtime,
                format!("Expression evaluation nesting too deep (limit {MAX_EVAL_DEPTH}). Possible infinite recursion."),
                Span::new(0, 0, 0, 0),
            ));
            return Ok(ArtValue::none());
        }
        let result = self.evaluate_inner(expr);
        self.eval_depth -= 1;
        result
    }

    pub(super) fn evaluate_inner(&mut self, expr: Expr) -> Result<ArtValue> {
        match expr {
            Expr::InterpolatedString(parts) => {
                use crate::fstring::eval_fstring;
                eval_fstring(parts, |e| self.evaluate(e))
            }
            Expr::Try(inner) => {
                // Com a introdução de weak/unowned, Try original de Result permanece como compat.
                let result_val = self.evaluate(*inner)?;
                match result_val {
                    ArtValue::EnumInstance {
                        enum_name,
                        variant,
                        mut values,
                    } if enum_name == "Result" => {
                        if variant == "Ok" {
                            Ok(values.pop().unwrap_or(ArtValue::none()))
                        } else {
                            Err(RuntimeError::Return(
                                values.pop().unwrap_or(ArtValue::none()),
                            ))
                        }
                    }
                    other => Ok(other),
                }
            }
            Expr::Literal(value) => Ok(value),
            Expr::Grouping { expression } => self.evaluate(*expression),
            Expr::Variable { name } => {
                let name_str = name.lexeme.clone();
                if (name_str == "variant" || name_str == "values")
                    && let Some(ArtValue::EnumInstance {
                        variant, values, ..
                    }) = self.environment.borrow().get("self")
                {
                    if name_str == "variant" {
                        return Ok(ArtValue::String(core::intern_arc(variant.as_str())));
                    } else {
                        return Ok(ArtValue::Array(values.clone()));
                    }
                }
                // Fallback para builtins: checamos ANTES para evitar empréstimos mutáveis desnecessários
                // do ambiente global (evita pânicos de RefCell em atores).
                // Nota: Shadowing ainda funciona se o usuário definir explicitamente a variável,
                // pois o ambiente local será checado em seguida.
                if let Some(builtin) = PRELUDE_VALUES.with(|p| p.get(name_str.as_str()).cloned()) {
                    // Mas espera! Se houver uma variável local com esse nome, ela deve ter precedência.
                    // Fazemos um borrow imutável rápido para verificar.
                    if !self.environment.borrow().has_locally(&name_str) {
                        return Ok(builtin);
                    }
                }

                let val = self.environment.borrow_mut().read_for_eval(&name_str);
                if let Some(v) = val {
                    if let ArtValue::MovedCapability = v {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!(
                                "Capability '{}' was already moved/consumed and cannot be reused",
                                name_str
                            ),
                            Span::new(name.start, name.end, name.line, name.col),
                        ));
                        return Ok(ArtValue::none());
                    }
                    return Ok(v);
                }

                // Não encontrado nem no ambiente léxico nem nos builtins
                let env_borrow = self.environment.borrow();
                let candidates = env_borrow.values.keys().copied();
                let suggestion = if let Some(best) = did_you_mean(&name_str, candidates) {
                    format!(" Did you mean '{}'?", best)
                } else {
                    String::new()
                };

                self.diagnostics.push(Diagnostic::new(
                    DiagnosticKind::Runtime,
                    format!("Undefined variable '{}'.{}", name_str, suggestion),
                    Span::new(name.start, name.end, name.line, name.col),
                ));
                Ok(ArtValue::none())
            }
            Expr::Unary { operator, right } => {
                let right_val = self.evaluate(*right)?;
                match operator.token_type {
                    core::TokenType::Minus => match right_val {
                        ArtValue::Int(n) => Ok(ArtValue::Int(-n)),
                        ArtValue::Float(f) => Ok(ArtValue::Float(-f)),
                        _ => {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "Type mismatch.".to_string(),
                                Span::new(
                                    operator.start,
                                    operator.end,
                                    operator.line,
                                    operator.col,
                                ),
                            ));
                            Ok(ArtValue::none())
                        }
                    },
                    core::TokenType::Bang => Ok(ArtValue::Bool(!self.is_truthy(&right_val))),
                    _ => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "Invalid operator.".to_string(),
                            Span::new(operator.start, operator.end, operator.line, operator.col),
                        ));
                        Ok(ArtValue::none())
                    }
                }
            }
            Expr::Logical {
                left,
                operator,
                right,
            } => {
                let left_val = self.evaluate(*left)?;
                if operator.token_type == core::TokenType::Or {
                    if self.is_truthy(&left_val) {
                        return Ok(left_val);
                    }
                } else if !self.is_truthy(&left_val) {
                    return Ok(left_val);
                }
                self.evaluate(*right)
            }
            Expr::Binary {
                left,
                operator,
                right,
            } => {
                let left_val = self.evaluate(*left)?;
                let right_val = self.evaluate(*right)?;
                match operator.token_type {
                    core::TokenType::Plus => match (&left_val, &right_val) {
                        (ArtValue::Int(l), ArtValue::Int(r)) => Ok(ArtValue::Int(l + r)),
                        (ArtValue::Float(l), ArtValue::Float(r)) => Ok(ArtValue::Float(l + r)),
                        (ArtValue::String(l), ArtValue::String(r)) => Ok(ArtValue::String(
                            std::sync::Arc::from(format!("{}{}", l, r)),
                        )),
                        (ArtValue::Int(l), ArtValue::Float(r)) => {
                            Ok(ArtValue::Float(*l as f64 + r))
                        }
                        (ArtValue::Float(l), ArtValue::Int(r)) => {
                            Ok(ArtValue::Float(l + *r as f64))
                        }
                        _ => {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "Type mismatch.".to_string(),
                                Span::new(
                                    operator.start,
                                    operator.end,
                                    operator.line,
                                    operator.col,
                                ),
                            ));
                            Ok(ArtValue::none())
                        }
                    },
                    core::TokenType::Minus => self.binary_num_op(left_val, right_val, |a, b| a - b),
                    core::TokenType::Star => self.binary_num_op(left_val, right_val, |a, b| a * b),
                    core::TokenType::Slash => {
                        let div_by_zero = matches!(right_val, ArtValue::Int(0))
                            || matches!(right_val, ArtValue::Float(f) if f == 0.0);
                        if div_by_zero {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "Division by zero".to_string(),
                                Span::new(
                                    operator.start,
                                    operator.end,
                                    operator.line,
                                    operator.col,
                                ),
                            ));
                            Ok(ArtValue::none())
                        } else {
                            self.binary_num_op(left_val, right_val, |a, b| a / b)
                        }
                    }
                    core::TokenType::Greater => {
                        self.binary_cmp_op(left_val, right_val, |a, b| a > b)
                    }
                    core::TokenType::GreaterEqual => {
                        self.binary_cmp_op(left_val, right_val, |a, b| a >= b)
                    }
                    core::TokenType::Less => self.binary_cmp_op(left_val, right_val, |a, b| a < b),
                    core::TokenType::LessEqual => {
                        self.binary_cmp_op(left_val, right_val, |a, b| a <= b)
                    }
                    core::TokenType::BangEqual => {
                        Ok(ArtValue::Bool(!self.is_equal(&left_val, &right_val)))
                    }
                    core::TokenType::EqualEqual => {
                        Ok(ArtValue::Bool(self.is_equal(&left_val, &right_val)))
                    }
                    _ => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "Invalid operator.".to_string(),
                            Span::new(operator.start, operator.end, operator.line, operator.col),
                        ));
                        Ok(ArtValue::none())
                    }
                }
            }
            Expr::Call {
                callee,
                type_args,
                arguments,
            } => self.handle_call(*callee, type_args, arguments),
            Expr::Tuple(elements) => {
                let mut evaluated_elements = Vec::new();
                for element_expr in elements {
                    let value = self.evaluate(element_expr)?;
                    self.note_composite_child(&value);
                    evaluated_elements.push(value);
                }
                // Tuple are heap allocated like arrays for passing by reference
                Ok(self.heapify_composite(ArtValue::Tuple(evaluated_elements)))
            }
            Expr::StructInit { name, fields } => {
                let struct_def = match self.type_registry.get_struct(&name.lexeme) {
                    Some(def) => def.clone(),
                    None => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("Undefined struct '{}'.", name.lexeme),
                            Span::new(name.start, name.end, name.line, name.col),
                        ));
                        return Ok(ArtValue::none().clone());
                    }
                };
                let mut field_values = HashMap::new();
                for (field_name, field_expr) in fields {
                    let value = self.evaluate(field_expr)?;
                    self.note_composite_child(&value);
                    field_values.insert(field_name.lexeme, value);
                }
                for (field_name, _field_type) in &struct_def.fields {
                    if !field_values.contains_key(field_name) {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("Missing field '{}'.", field_name),
                            Span::new(name.start, name.end, name.line, name.col),
                        ));
                        return Ok(ArtValue::none().clone());
                    }
                }
                Ok(self.heapify_composite(ArtValue::StructInstance {
                    struct_name: name.lexeme,
                    fields: field_values,
                }))
            }
            Expr::EnumInit {
                name,
                variant,
                values,
            } => {
                let enum_name = match name {
                    Some(n) => n.lexeme,
                    None => {
                        // Inferência: procurar enum que contenha a variant de forma única
                        let mut candidate: Option<String> = None;
                        for (ename, edef) in self.type_registry.enums.iter() {
                            if edef.variants.iter().any(|(v, _)| v == &variant.lexeme) {
                                if candidate.is_some() && candidate.as_ref() != Some(ename) {
                                    self.diagnostics.push(Diagnostic::new(
                                        DiagnosticKind::Runtime,
                                        "Ambiguous enum variant shorthand.".to_string(),
                                        Span::new(
                                            variant.start,
                                            variant.end,
                                            variant.line,
                                            variant.col,
                                        ),
                                    ));
                                    return Ok(ArtValue::none());
                                }
                                candidate = Some(ename.clone());
                            }
                        }
                        match candidate {
                            Some(c) => c,
                            None => {
                                self.diagnostics.push(Diagnostic::new(
                                    DiagnosticKind::Runtime,
                                    "Cannot infer enum type for shorthand initialization."
                                        .to_string(),
                                    Span::new(
                                        variant.start,
                                        variant.end,
                                        variant.line,
                                        variant.col,
                                    ),
                                ));
                                return Ok(ArtValue::none());
                            }
                        }
                    }
                };
                let enum_def = match self.type_registry.get_enum(&enum_name) {
                    Some(def) => def.clone(),
                    None => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("Undefined enum '{}'.", enum_name),
                            Span::new(variant.start, variant.end, variant.line, variant.col),
                        ));
                        return Ok(ArtValue::none());
                    }
                };
                let variant_def = match enum_def
                    .variants
                    .iter()
                    .find(|(v_name, _)| v_name == &variant.lexeme)
                {
                    Some(v) => v,
                    None => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("Invalid enum variant '{}'.", variant.lexeme),
                            Span::new(variant.start, variant.end, variant.line, variant.col),
                        ));
                        return Ok(ArtValue::none());
                    }
                };
                let mut evaluated_values = Vec::new();
                for value_expr in values {
                    let v = self.evaluate(value_expr)?;
                    self.note_composite_child(&v);
                    evaluated_values.push(v);
                }
                match &variant_def.1 {
                    Some(expected_params) => {
                        if evaluated_values.len() != expected_params.len() {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "Wrong number of arguments.".to_string(),
                                Span::new(variant.start, variant.end, variant.line, variant.col),
                            ));
                            return Ok(ArtValue::none());
                        }
                    }
                    None => {
                        if !evaluated_values.is_empty() {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "Wrong number of arguments.".to_string(),
                                Span::new(variant.start, variant.end, variant.line, variant.col),
                            ));
                            return Ok(ArtValue::none());
                        }
                    }
                }
                Ok(self.heapify_composite(ArtValue::EnumInstance {
                    enum_name,
                    variant: variant.lexeme,
                    values: evaluated_values,
                }))
            }
            Expr::FieldAccess { object, field } => {
                let evaluated = self.evaluate(*object)?;
                let obj_value_ref = self.resolve_composite(&evaluated).clone();
                // Normalize internal Optional representation to the language-level Option enum.
                let obj_value = match obj_value_ref {
                    ArtValue::Optional(boxed) => {
                        if let Some(v) = &*boxed {
                            ArtValue::EnumInstance {
                                enum_name: "Option".to_string(),
                                variant: "Some".to_string(),
                                values: vec![v.clone()],
                            }
                        } else {
                            ArtValue::EnumInstance {
                                enum_name: "Option".to_string(),
                                variant: "None".to_string(),
                                values: Vec::new(),
                            }
                        }
                    }
                    other => other,
                };
                use crate::field_access::{enum_method, struct_field_or_method};
                match obj_value {
                    ArtValue::Array(arr) => match field.lexeme.as_str() {
                        "sum" => {
                            let mut sum = 0;
                            for val in arr.iter() {
                                if let ArtValue::Int(n) = val {
                                    sum += n;
                                } else {
                                    self.diagnostics.push(Diagnostic::new(
                                        DiagnosticKind::Runtime,
                                        "Type mismatch in sum (expected Int)".to_string(),
                                        Span::new(field.start, field.end, field.line, field.col),
                                    ));
                                    return Ok(ArtValue::none());
                                }
                            }
                            Ok(ArtValue::Int(sum))
                        }
                        "count" => Ok(ArtValue::Int(arr.len() as i64)),
                        _ => {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "Type mismatch.".to_string(),
                                Span::new(field.start, field.end, field.line, field.col),
                            ));
                            Ok(ArtValue::none())
                        }
                    },
                    ArtValue::StructInstance {
                        struct_name,
                        fields,
                    } => {
                        if let Some(v) = struct_field_or_method(
                            &struct_name,
                            &fields,
                            &field,
                            &self.type_registry,
                        ) {
                            Ok(v)
                        } else {
                            let available = fields.keys().map(String::as_str);
                            let suggestion =
                                if let Some(best) = did_you_mean(&field.lexeme, available) {
                                    format!(" Did you mean '{}'?", best)
                                } else {
                                    String::new()
                                };

                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                format!(
                                    "Missing field '{}' on struct '{}'.{}",
                                    field.lexeme, struct_name, suggestion
                                ),
                                Span::new(field.start, field.end, field.line, field.col),
                            ));
                            Ok(ArtValue::none())
                        }
                    }
                    ArtValue::EnumInstance {
                        enum_name,
                        variant,
                        values,
                    } => {
                        if let Some(v) =
                            enum_method(&enum_name, &variant, &values, &field, &self.type_registry)
                        {
                            Ok(v)
                        } else {
                            // Suggest methods on the enum variant (since enum instances only have methods/values)
                            // We can check the type registry for methods on this enum type
                            let available = self
                                .type_registry
                                .get_enum(&enum_name)
                                .map(|def| {
                                    def.methods.keys().map(String::as_str).collect::<Vec<_>>()
                                })
                                .unwrap_or_default();

                            let suggestion = if let Some(best) =
                                did_you_mean(&field.lexeme, available.into_iter())
                            {
                                format!(" Did you mean '{}'?", best)
                            } else {
                                String::new()
                            };

                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                format!(
                                    "Missing field or method '{}' on enum '{}'.{}",
                                    field.lexeme, enum_name, suggestion
                                ),
                                Span::new(field.start, field.end, field.line, field.col),
                            ));
                            Ok(ArtValue::none())
                        }
                    }
                    _ => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "Type mismatch.".to_string(),
                            Span::new(field.start, field.end, field.line, field.col),
                        ));
                        Ok(ArtValue::none())
                    }
                }
            }
            Expr::Weak(inner) => {
                // Açúcar: weak expr => builtin weak(expr)
                let expr = Expr::Call {
                    callee: Box::new(Expr::Variable {
                        name: Token::dummy("weak"),
                    }),
                    type_args: None,
                    arguments: vec![*inner],
                };
                self.evaluate(expr)
            }
            Expr::Unowned(inner) => {
                let expr = Expr::Call {
                    callee: Box::new(Expr::Variable {
                        name: Token::dummy("unowned"),
                    }),
                    type_args: None,
                    arguments: vec![*inner],
                };
                self.evaluate(expr)
            }
            Expr::WeakUpgrade(inner) => {
                // Açúcar: expr? -> weak_get(expr)
                let expr = Expr::Call {
                    callee: Box::new(Expr::Variable {
                        name: Token::dummy("weak_get"),
                    }),
                    type_args: None,
                    arguments: vec![*inner],
                };
                self.evaluate(expr)
            }
            Expr::UnownedAccess(inner) => {
                // Açúcar: expr! -> unowned_get(expr)
                let expr = Expr::Call {
                    callee: Box::new(Expr::Variable {
                        name: Token::dummy("unowned_get"),
                    }),
                    type_args: None,
                    arguments: vec![*inner],
                };
                self.evaluate(expr)
            }
            Expr::Cast { object, .. } => self.evaluate(*object),
            Expr::Array(elements) => {
                let mut evaluated_elements = Vec::new();
                for element in elements {
                    let v = self.evaluate(element)?;
                    self.note_composite_child(&v);
                    evaluated_elements.push(v);
                }
                Ok(self.heapify_composite(ArtValue::Array(evaluated_elements)))
            }
            Expr::SpawnActor { body } => {
                // Create a new actor from an expression context and return its handle
                let aid = self.next_actor_id;
                self.next_actor_id += 1;
                let actor_env = Rc::new(RefCell::new(Environment::new(
                    Some(self.environment.clone()),
                    0,
                    None,
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
                Ok(ArtValue::Actor(aid))
            }

            Expr::Template(_) => {
                // ArtML templates are not evaluated by the interpreter — they target the JS codegen.
                Err(RuntimeError::TypeError(
                    "ArtML templates can only be used with `art build --target js`".to_string(),
                ))
            }
        }
    }

    pub(super) fn handle_call(
        &mut self,
        callee: Expr,
        type_args: Option<Vec<String>>,
        arguments: Vec<Expr>,
    ) -> Result<ArtValue> {
        if let Expr::Variable { name } = &callee {
            let is_defined = self.environment.borrow().get(&name.lexeme).is_some();
            if !is_defined {
                // Também checamos nos builtins antes de cair para o shell.
                // Como não estão mais no environment (otimização de cold-start),
                // precisamos verificar o registro global aqui.
                let is_builtin = PRELUDE_VALUES.with(|p| p.contains_key(name.lexeme.as_str()));
                if !is_builtin {
                    return self.run_shell_function_call(&name.lexeme, arguments);
                }
            }
        }

        if let Expr::FieldAccess { field, .. } = &callee {
            self.call_span = Span::new(field.start, field.end, field.line, field.col);
        }

        let original_expr = callee.clone();
        let value = self.evaluate(callee)?;
        match value {
            ArtValue::Function(func) => self.call_function(func, type_args, arguments),
            ArtValue::Builtin(b) => self.call_builtin(b, arguments),
            ArtValue::EnumInstance {
                enum_name,
                variant,
                values,
            } if values.is_empty() => self.construct_enum_variant(enum_name, variant, arguments),
            other => self.call_fallback(original_expr, other, &arguments),
        }
    }

    pub(super) fn call_function(
        &mut self,
        func: Rc<Function>,
        type_args: Option<Vec<String>>,
        arguments: Vec<Expr>,
    ) -> Result<ArtValue> {
        // record call counter by function name (if present)
        let callee_name_opt = func.name.clone();
        if let Some(name) = &callee_name_opt {
            self.call_counters
                .entry(name.clone())
                .and_modify(|c| *c += 1)
                .or_insert(1);
        }
        // record edge from caller -> callee
        if let Some(caller) = self.fn_stack.last().and_then(|opt| opt.clone())
            && let Some(callee) = &callee_name_opt
        {
            let edge = format!("{}->{}", caller, callee);
            self.edge_counters
                .entry(edge)
                .and_modify(|c| *c += 1)
                .or_insert(1);
        }
        self.fn_stack.push(callee_name_opt.clone());

        let argc = arguments.len();
        if func.params.len() != argc {
            self.diagnostics.push(Diagnostic::new(
                DiagnosticKind::Runtime,
                "Wrong number of arguments.".to_string(),
                Span::new(0, 0, 0, 0),
            ));
            self.fn_stack.pop();
            return Ok(ArtValue::none());
        }

        // Avalia argumentos
        let mut evaluated_args = Vec::with_capacity(argc);
        for arg in arguments {
            evaluated_args.push(self.evaluate(arg)?);
        }

        // Generic type parameter validation
        if let Some(type_params) = &func.type_params
            && !type_params.is_empty()
        {
            // Infer concrete types: explicit type_args take priority, else infer from values
            let concrete: Vec<String> = if let Some(explicit) = &type_args {
                explicit.clone()
            } else {
                // For each type param, find the first function param annotated with that name
                // and use the runtime type of the corresponding argument.
                type_params
                    .iter()
                    .map(|(tname, _)| {
                        func.params
                            .iter()
                            .position(|p| p.ty.as_deref() == Some(tname.as_str()))
                            .and_then(|idx| evaluated_args.get(idx))
                            .map(|v| v.type_name())
                            .unwrap_or_else(|| "Unknown".to_string())
                    })
                    .collect()
            };
            // Validate constraints
            for (i, (tname, constraint)) in type_params.iter().enumerate() {
                if let Some(bound) = constraint {
                    let concrete_ty = concrete.get(i).map(String::as_str).unwrap_or("Unknown");
                    let ok = match bound.as_str() {
                        "Numeric" => matches!(concrete_ty, "Int" | "Float"),
                        "Eq" | "Hash" => {
                            matches!(concrete_ty, "Int" | "Float" | "String" | "Bool")
                        }
                        "Comparable" => {
                            matches!(concrete_ty, "Int" | "Float" | "String")
                        }
                        _ => true,
                    };
                    if !ok {
                        self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                format!(
                                    "Type '{}' does not satisfy constraint '{}' for type parameter '{}'",
                                    concrete_ty, bound, tname
                                ),
                                self.call_span,
                            ));
                    }
                }
            }
        }

        let previous_env = self.environment.clone();
        let base_env = match func.closure.upgrade() {
            Some(env) => env,
            None => {
                self.diagnostics.push(Diagnostic::new(
                    DiagnosticKind::Runtime,
                    "Dangling closure environment".to_string(),
                    Span::new(0, 0, 0, 0),
                ));
                Rc::new(RefCell::new(Environment::new(None, 0, None)))
            }
        };

        // Arenas Implícitas para Funções (Adaptive ARC)
        let mut pushed_arena = false;
        let call_arena = if self.arena_with_active {
            self.current_arena
        } else {
            let aid = self.push_implicit_arena();
            pushed_arena = true;
            Some(aid)
        };

        let call_env = Rc::new(RefCell::new(Environment::new(
            Some(base_env.clone()),
            base_env.borrow().depth + 1,
            call_arena,
        )));
        self.environment = call_env.clone();

        // Bind parâmetros
        for (param, mut value) in func.params.iter().zip(evaluated_args) {
            let target_aid = call_env.borrow().associated_arena;
            self.promote_if_escaping(target_aid, &mut value);
            self.environment
                .borrow_mut()
                .define(&param.name.lexeme, value);
        }

        let result = self.execute(Rc::as_ref(&func.body).clone());

        let mut return_val = match result {
            Ok(()) => ArtValue::none(),
            Err(RuntimeError::Return(mut rv)) => {
                // Ao retornar de uma função, se estiver saindo de uma arena para uma externa, promovemos.
                let target_aid = previous_env.borrow().associated_arena;
                self.promote_if_escaping(target_aid, &mut rv);
                rv
            }
            Err(e) => {
                self.pop_implicit_arena();
                self.environment = previous_env;
                self.fn_stack.pop();
                return Err(e);
            }
        };

        // Escape analysis para closures
        if let ArtValue::Function(f) = &return_val
            && f.retained_env.is_none()
            && let Some(captured_env) = f.closure.upgrade()
        {
            let escaped = Function {
                name: f.name.clone(),
                type_params: f.type_params.clone(),
                params: f.params.clone(),
                body: f.body.clone(),
                closure: f.closure.clone(),
                retained_env: Some(captured_env),
            };
            return_val = ArtValue::Function(Rc::new(escaped));
        }

        self.drop_scope_heap_objects(&call_env);
        if pushed_arena {
            self.pop_implicit_arena();
        }
        self.environment = previous_env;
        self.fn_stack.pop();

        Ok(return_val)
    }
}
