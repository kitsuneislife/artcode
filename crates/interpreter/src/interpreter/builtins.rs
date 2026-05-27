use super::Interpreter;
use crate::values::Result;
use core::ast::{ArtValue, Expr, ObjHandle};
use diagnostics::{Diagnostic, DiagnosticKind};
use std::collections::HashMap;
use std::sync::Arc;

impl Interpreter {
    pub(super) fn call_builtin(&mut self, b: core::ast::BuiltinFn, arguments: Vec<Expr>) -> Result<ArtValue> {
        match b {
            core::ast::BuiltinFn::Println => {
                if !self.ensure_pure_allowed("println") {
                    return Ok(ArtValue::none());
                }
                if let Some(first) = arguments.into_iter().next() {
                    let val = self.evaluate(first)?;
                    println!("{}", val);
                } else {
                    println!();
                }
                Ok(ArtValue::none())
            }
            core::ast::BuiltinFn::EnumIsOk(val) => {
                let is_ok = if let ArtValue::EnumInstance { variant, .. } = &*val {
                    variant == "Ok" || variant == "Some"
                } else {
                    false
                };
                Ok(ArtValue::Bool(is_ok))
            }
            core::ast::BuiltinFn::EnumIsErr(val) => {
                let is_err = if let ArtValue::EnumInstance { variant, .. } = &*val {
                    variant == "Err" || variant == "None"
                } else {
                    false
                };
                Ok(ArtValue::Bool(is_err))
            }
            core::ast::BuiltinFn::EnumUnwrap(val) => {
                if let ArtValue::EnumInstance {
                    variant, values, ..
                } = &*val
                {
                    if variant == "Ok" || variant == "Some" {
                        Ok(values.first().cloned().unwrap_or_else(ArtValue::none))
                    } else {
                        // Produce diagnostic and return error
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "Called `unwrap()` on an `Err` or `None` value.".to_string(),
                            self.call_span,
                        ));
                        Ok(ArtValue::none())
                    }
                } else {
                    Ok(ArtValue::none())
                }
            }
            core::ast::BuiltinFn::EnumUnwrapOr(val) => {
                if let ArtValue::EnumInstance {
                    variant, values, ..
                } = &*val
                {
                    if variant == "Ok" || variant == "Some" {
                        Ok(values.first().cloned().unwrap_or_else(ArtValue::none))
                    } else {
                        if arguments.len() == 1 {
                            self.evaluate(arguments[0].clone())
                        } else {
                            Ok(ArtValue::none())
                        }
                    }
                } else {
                    Ok(ArtValue::none())
                }
            }
            core::ast::BuiltinFn::MapNew => Ok(ArtValue::Map(core::ast::MapRef(
                std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            ))),
            core::ast::BuiltinFn::MapSet => {
                let mut args = arguments.into_iter();
                if let (Some(map_expr), Some(key_expr), Some(val_expr)) =
                    (args.next(), args.next(), args.next())
                {
                    let map_val = self.evaluate(map_expr)?;
                    let key_val = self.evaluate(key_expr)?;
                    let v = self.evaluate(val_expr)?;
                    if let (ArtValue::Map(m), ArtValue::String(k)) = (map_val, key_val) {
                        m.0.lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .insert(k.to_string(), v);
                        Ok(ArtValue::none())
                    } else {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "map_set: invalid arguments".to_string(),
                            self.call_span,
                        ));
                        Ok(ArtValue::none())
                    }
                } else {
                    Ok(ArtValue::none())
                }
            }
            core::ast::BuiltinFn::MapGet => {
                let mut args = arguments.into_iter();
                if let (Some(map_expr), Some(key_expr)) = (args.next(), args.next()) {
                    let map_val = self.evaluate(map_expr)?;
                    let key_val = self.evaluate(key_expr)?;
                    if let (ArtValue::Map(m), ArtValue::String(k)) = (map_val, key_val) {
                        let map = m.0.lock().unwrap_or_else(|e| e.into_inner());
                        if let Some(v) = map.get(k.as_ref()) {
                            Ok(ArtValue::Optional(Box::new(Some(v.clone()))))
                        } else {
                            Ok(ArtValue::none())
                        }
                    } else {
                        Ok(ArtValue::none())
                    }
                } else {
                    Ok(ArtValue::none())
                }
            }
            core::ast::BuiltinFn::MapHas => {
                let mut args = arguments.into_iter();
                if let (Some(map_expr), Some(key_expr)) = (args.next(), args.next()) {
                    let map_val = self.evaluate(map_expr)?;
                    let key_val = self.evaluate(key_expr)?;
                    if let (ArtValue::Map(m), ArtValue::String(k)) = (map_val, key_val) {
                        Ok(ArtValue::Bool(
                            m.0.lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .contains_key(k.as_ref()),
                        ))
                    } else {
                        Ok(ArtValue::Bool(false))
                    }
                } else {
                    Ok(ArtValue::Bool(false))
                }
            }
            core::ast::BuiltinFn::StreamNew => {
                if arguments.len() != 1 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "stream expects exactly one array argument".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }
                let input = self.evaluate(arguments[0].clone())?;
                let resolved = self.resolve_composite(&input).clone();
                match resolved {
                    ArtValue::Array(source) => Ok(self.build_stream_value(source, Vec::new())),
                    _ => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "stream expects an array argument".to_string(),
                            self.call_span,
                        ));
                        Ok(ArtValue::none())
                    }
                }
            }
            core::ast::BuiltinFn::StreamMap => {
                if arguments.len() != 2 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "map expects (stream, callable)".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }
                let stream_value = self.evaluate(arguments[0].clone())?;
                let callable = self.evaluate(arguments[1].clone())?;
                match self.decode_stream_value(stream_value) {
                    Ok((source, mut ops)) => {
                        ops.push(Self::stream_op("map", callable));
                        Ok(self.build_stream_value(source, ops))
                    }
                    Err(msg) => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            msg,
                            self.call_span,
                        ));
                        Ok(ArtValue::none())
                    }
                }
            }
            core::ast::BuiltinFn::StreamFilter => {
                if arguments.len() != 2 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "filter expects (stream, callable)".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }
                let stream_value = self.evaluate(arguments[0].clone())?;
                let callable = self.evaluate(arguments[1].clone())?;
                match self.decode_stream_value(stream_value) {
                    Ok((source, mut ops)) => {
                        ops.push(Self::stream_op("filter", callable));
                        Ok(self.build_stream_value(source, ops))
                    }
                    Err(msg) => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            msg,
                            self.call_span,
                        ));
                        Ok(ArtValue::none())
                    }
                }
            }
            core::ast::BuiltinFn::StreamCollect => {
                if arguments.len() != 1 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "collect expects a stream argument".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }
                let stream_value = self.evaluate(arguments[0].clone())?;
                match self.decode_stream_value(stream_value) {
                    Ok((source, ops)) => {
                        let collected = self.run_stream_pipeline(source, ops)?;
                        Ok(ArtValue::Array(collected))
                    }
                    Err(msg) => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            msg,
                            self.call_span,
                        ));
                        Ok(ArtValue::none())
                    }
                }
            }
            core::ast::BuiltinFn::StreamCount => {
                if arguments.len() != 1 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "count expects a stream argument".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }
                let stream_value = self.evaluate(arguments[0].clone())?;
                match self.decode_stream_value(stream_value) {
                    Ok((source, ops)) => {
                        let collected = self.run_stream_pipeline(source, ops)?;
                        Ok(ArtValue::Int(collected.len() as i64))
                    }
                    Err(msg) => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            msg,
                            self.call_span,
                        ));
                        Ok(ArtValue::none())
                    }
                }
            }
            core::ast::BuiltinFn::SetNew => Ok(ArtValue::Set(core::ast::SetRef(
                std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            ))),
            core::ast::BuiltinFn::SetAdd => {
                let mut args = arguments.into_iter();
                if let (Some(set_expr), Some(val_expr)) = (args.next(), args.next()) {
                    let set_val = self.evaluate(set_expr)?;
                    let v = self.evaluate(val_expr)?;
                    if let ArtValue::Set(s) = set_val {
                        let mut set = s.0.lock().unwrap_or_else(|e| e.into_inner());
                        if !set.contains(&v) {
                            set.push(v);
                        }
                        Ok(ArtValue::none())
                    } else {
                        Ok(ArtValue::none())
                    }
                } else {
                    Ok(ArtValue::none())
                }
            }
            core::ast::BuiltinFn::SetHas => {
                let mut args = arguments.into_iter();
                if let (Some(set_expr), Some(val_expr)) = (args.next(), args.next()) {
                    let set_val = self.evaluate(set_expr)?;
                    let v = self.evaluate(val_expr)?;
                    if let ArtValue::Set(s) = set_val {
                        Ok(ArtValue::Bool(
                            s.0.lock().unwrap_or_else(|e| e.into_inner()).contains(&v),
                        ))
                    } else {
                        Ok(ArtValue::Bool(false))
                    }
                } else {
                    Ok(ArtValue::Bool(false))
                }
            }
            core::ast::BuiltinFn::MathAbs => {
                if let Some(first) = arguments.into_iter().next() {
                    match self.evaluate(first)? {
                        ArtValue::Int(i) => Ok(ArtValue::Int(i.abs())),
                        ArtValue::Float(f) => Ok(ArtValue::Float(f.abs())),
                        _ => Ok(ArtValue::none()),
                    }
                } else {
                    Ok(ArtValue::none())
                }
            }
            core::ast::BuiltinFn::MathPow => {
                let mut args = arguments.into_iter();
                if let (Some(base_expr), Some(exp_expr)) = (args.next(), args.next()) {
                    match (self.evaluate(base_expr)?, self.evaluate(exp_expr)?) {
                        (ArtValue::Int(base), ArtValue::Int(exp)) => {
                            if exp >= 0 {
                                Ok(ArtValue::Int(base.pow(exp as u32)))
                            } else {
                                Ok(ArtValue::none())
                            }
                        }
                        (ArtValue::Float(base), ArtValue::Float(exp)) => {
                            Ok(ArtValue::Float(base.powf(exp)))
                        }
                        (ArtValue::Int(base), ArtValue::Float(exp)) => {
                            Ok(ArtValue::Float((base as f64).powf(exp)))
                        }
                        (ArtValue::Float(base), ArtValue::Int(exp)) => {
                            Ok(ArtValue::Float(base.powi(exp as i32)))
                        }
                        _ => Ok(ArtValue::none()),
                    }
                } else {
                    Ok(ArtValue::none())
                }
            }
            core::ast::BuiltinFn::MathClamp => {
                let mut args = arguments.into_iter();
                if let (Some(val_expr), Some(min_expr), Some(max_expr)) =
                    (args.next(), args.next(), args.next())
                {
                    match (
                        self.evaluate(val_expr)?,
                        self.evaluate(min_expr)?,
                        self.evaluate(max_expr)?,
                    ) {
                        (ArtValue::Int(v), ArtValue::Int(min), ArtValue::Int(max)) => {
                            Ok(ArtValue::Int(v.clamp(min, max)))
                        }
                        (ArtValue::Float(v), ArtValue::Float(min), ArtValue::Float(max)) => {
                            Ok(ArtValue::Float(v.clamp(min, max)))
                        }
                        _ => Ok(ArtValue::none()),
                    }
                } else {
                    Ok(ArtValue::none())
                }
            }
            core::ast::BuiltinFn::DagTopoSort => {
                fn as_array(interp: &Interpreter, v: ArtValue) -> Option<Vec<ArtValue>> {
                    match v {
                        ArtValue::Array(a) => Some(a),
                        ArtValue::HeapComposite(h) => interp
                            .heap_objects
                            .get(&h.0)
                            .map(|o| o.value.clone())
                            .and_then(|ov| match ov {
                                ArtValue::Array(a) => Some(a),
                                _ => None,
                            }),
                        _ => None,
                    }
                }

                fn as_tuple2(interp: &Interpreter, v: ArtValue) -> Option<(ArtValue, ArtValue)> {
                    match v {
                        ArtValue::Tuple(items) if items.len() == 2 => {
                            Some((items[0].clone(), items[1].clone()))
                        }
                        ArtValue::HeapComposite(h) => interp
                            .heap_objects
                            .get(&h.0)
                            .map(|o| o.value.clone())
                            .and_then(|ov| match ov {
                                ArtValue::Tuple(items) if items.len() == 2 => {
                                    Some((items[0].clone(), items[1].clone()))
                                }
                                _ => None,
                            }),
                        _ => None,
                    }
                }

                fn as_string(v: &ArtValue) -> Option<String> {
                    match v {
                        ArtValue::String(s) => Some(s.to_string()),
                        _ => None,
                    }
                }

                let mut args = arguments.into_iter();
                let (Some(nodes_expr), Some(deps_expr)) = (args.next(), args.next()) else {
                    return Ok(ArtValue::none());
                };

                let nodes_val = self.evaluate(nodes_expr)?;
                let deps_val = self.evaluate(deps_expr)?;

                let Some(node_items) = as_array(self, nodes_val) else {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "dag_topo_sort: first argument must be an array of strings".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                };
                let Some(dep_items) = as_array(self, deps_val) else {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "dag_topo_sort: second argument must be an array of tuples (node, depends_on)"
                            .to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                };

                let mut indeg: HashMap<String, usize> = HashMap::new();
                let mut adj: HashMap<String, Vec<String>> = HashMap::new();

                for n in &node_items {
                    let Some(name) = as_string(n) else {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "dag_topo_sort: nodes array must contain only strings".to_string(),
                            self.call_span,
                        ));
                        return Ok(ArtValue::none());
                    };
                    indeg.entry(name.clone()).or_insert(0);
                    adj.entry(name).or_default();
                }

                for dep in dep_items {
                    let Some((node_v, dep_v)) = as_tuple2(self, dep) else {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "dag_topo_sort: dependency entries must be tuples (node, depends_on)"
                                .to_string(),
                            self.call_span,
                        ));
                        return Ok(ArtValue::none());
                    };
                    let (Some(node), Some(depends_on)) = (as_string(&node_v), as_string(&dep_v))
                    else {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "dag_topo_sort: dependency tuple values must be strings".to_string(),
                            self.call_span,
                        ));
                        return Ok(ArtValue::none());
                    };

                    // (node, depends_on) means: depends_on -> node
                    indeg.entry(node.clone()).or_insert(0);
                    indeg.entry(depends_on.clone()).or_insert(0);
                    adj.entry(depends_on.clone())
                        .or_default()
                        .push(node.clone());
                    adj.entry(node.clone()).or_default();
                    if let Some(v) = indeg.get_mut(&node) {
                        *v += 1;
                    }
                }

                let mut ready = std::collections::BTreeSet::new();
                for (n, d) in &indeg {
                    if *d == 0 {
                        ready.insert(n.clone());
                    }
                }

                let mut out: Vec<ArtValue> = Vec::new();
                while let Some(next) = ready.pop_first() {
                    out.push(ArtValue::String(Arc::from(next.clone())));
                    if let Some(children) = adj.get(&next).cloned() {
                        for child in children {
                            if let Some(d) = indeg.get_mut(&child)
                                && *d > 0
                            {
                                *d -= 1;
                                if *d == 0 {
                                    ready.insert(child);
                                }
                            }
                        }
                    }
                }

                if out.len() != indeg.len() {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "dag_topo_sort: cycle detected in dependency graph".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }

                Ok(self.heapify_composite(ArtValue::Array(out)))
            }
            core::ast::BuiltinFn::TimeNow => {
                if !self.ensure_pure_allowed("time_now") {
                    return Ok(ArtValue::none());
                }

                // TTD Replay intercept: Se estamos re-reproduzindo uma fita de eventos,
                // devemos retornar O MESMO valor gravado no instante real
                if let Some(replayer) = &mut self.replayer {
                    match replayer.consume_intercept("time_now", self.executed_statements) {
                        Ok(Some(payload)) => return Ok(payload),
                        Ok(None) => {} // ignora, not unexpected match
                        Err(e) => {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                e,
                                self.call_span,
                            ));
                            return Ok(ArtValue::none());
                        }
                    }
                }

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;

                if let Some(tracer) = &mut self.tracer {
                    let _ = tracer.record_event(
                        "time_now",
                        self.executed_statements,
                        ArtValue::Int(now),
                    );
                }

                Ok(ArtValue::Int(now))
            }
            core::ast::BuiltinFn::GCStats => {
                let mut stats = std::collections::HashMap::new();
                stats.insert(
                    "promotions".to_string(),
                    ArtValue::Int(self.finalizer_promotions as i64),
                );
                stats.insert(
                    "heap_objects".to_string(),
                    ArtValue::Int(self.heap_objects.len() as i64),
                );
                stats.insert(
                    "next_arena_id".to_string(),
                    ArtValue::Int(self.next_arena_id as i64),
                );
                Ok(ArtValue::StructInstance {
                    struct_name: "GCStats".to_string(),
                    fields: stats,
                })
            }
            core::ast::BuiltinFn::RuntimeVersion => {
                Ok(ArtValue::String(std::sync::Arc::from("0.2.0-adaptive-arc")))
            }
            core::ast::BuiltinFn::IOReadText => {
                if !self.ensure_pure_allowed("io_read_text") {
                    return Ok(ArtValue::none());
                }
                if let Some(first) = arguments.into_iter().next() {
                    if let ArtValue::String(path) = self.evaluate(first)? {
                        if let Ok(content) = std::fs::read_to_string(path.as_ref()) {
                            Ok(ArtValue::String(std::sync::Arc::from(content)))
                        } else {
                            Ok(ArtValue::none())
                        }
                    } else {
                        Ok(ArtValue::none())
                    }
                } else {
                    Ok(ArtValue::none())
                }
            }
            core::ast::BuiltinFn::IOWriteText => {
                if !self.ensure_pure_allowed("io_write_text") {
                    return Ok(ArtValue::Bool(false));
                }
                let mut args = arguments.into_iter();
                if let (Some(path_expr), Some(content_expr)) = (args.next(), args.next()) {
                    if let (ArtValue::String(path), ArtValue::String(content)) =
                        (self.evaluate(path_expr)?, self.evaluate(content_expr)?)
                    {
                        if std::fs::write(path.as_ref(), content.as_ref()).is_ok() {
                            Ok(ArtValue::Bool(true))
                        } else {
                            Ok(ArtValue::Bool(false))
                        }
                    } else {
                        Ok(ArtValue::Bool(false))
                    }
                } else {
                    Ok(ArtValue::Bool(false))
                }
            }
            core::ast::BuiltinFn::HttpGetText => {
                if !self.ensure_pure_allowed("http_get_text") {
                    return Ok(ArtValue::none());
                }

                let Some(first) = arguments.into_iter().next() else {
                    return Ok(ArtValue::none());
                };
                let ArtValue::String(url) = self.evaluate(first)? else {
                    return Ok(ArtValue::none());
                };

                let url_str = url.as_ref();
                let Some(rest) = url_str.strip_prefix("http://") else {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "http_get_text: only http:// URLs are supported".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                };

                let (host_port, path_part) = match rest.split_once('/') {
                    Some((h, p)) => (h, format!("/{}", p)),
                    None => (rest, "/".to_string()),
                };

                let (host, port) = match host_port.rsplit_once(':') {
                    Some((h, p)) => match p.parse::<u16>() {
                        Ok(port) => (h, port),
                        Err(_) => {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "http_get_text: invalid port in URL".to_string(),
                                self.call_span,
                            ));
                            return Ok(ArtValue::none());
                        }
                    },
                    None => (host_port, 80u16),
                };

                let request = format!(
                    "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
                    path_part, host
                );

                match std::net::TcpStream::connect((host, port)) {
                    Ok(mut stream) => {
                        if std::io::Write::write_all(&mut stream, request.as_bytes()).is_err() {
                            return Ok(ArtValue::none());
                        }
                        let mut response = String::new();
                        if std::io::Read::read_to_string(&mut stream, &mut response).is_err() {
                            return Ok(ArtValue::none());
                        }
                        if let Some((_, body)) = response.split_once("\r\n\r\n") {
                            Ok(ArtValue::String(Arc::from(body.to_string())))
                        } else {
                            Ok(ArtValue::String(Arc::from(response)))
                        }
                    }
                    Err(_) => Ok(ArtValue::none()),
                }
            }
            core::ast::BuiltinFn::RandomSeed => {
                if !self.ensure_pure_allowed("rand_seed") {
                    return Ok(ArtValue::none());
                }
                if let Some(first) = arguments.into_iter().next() {
                    if let ArtValue::Int(seed) = self.evaluate(first)? {
                        self.rng_state = seed as u64;
                        Ok(ArtValue::none())
                    } else {
                        Ok(ArtValue::none())
                    }
                } else {
                    Ok(ArtValue::none())
                }
            }
            core::ast::BuiltinFn::RandomNext => {
                if !self.ensure_pure_allowed("rand_next") {
                    return Ok(ArtValue::none());
                }
                // TTD Replay intercept: se estamos re-reproduzindo, usar valor gravado
                if let Some(replayer) = &mut self.replayer {
                    match replayer.consume_intercept("rand_next", self.executed_statements) {
                        Ok(Some(payload)) => return Ok(payload),
                        Ok(None) => {}
                        Err(e) => {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                e,
                                self.call_span,
                            ));
                            return Ok(ArtValue::none());
                        }
                    }
                }

                // Simple LCG
                self.rng_state = self
                    .rng_state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let rand_val = (self.rng_state >> 32) as i64;

                if let Some(tracer) = &mut self.tracer {
                    let _ = tracer.record_event(
                        "rand_next",
                        self.executed_statements,
                        ArtValue::Int(rand_val),
                    );
                }

                Ok(ArtValue::Int(
                    format!("{}", rand_val)
                        .trim_start_matches('-')
                        .parse()
                        .unwrap_or(rand_val),
                ))
            }
            core::ast::BuiltinFn::Len => {
                if let Some(first) = arguments.into_iter().next() {
                    let val = self.evaluate(first)?;
                    let n = match val {
                        ArtValue::String(ref s) => s.len() as i64,
                        ArtValue::Array(ref a) => a.len() as i64,
                        ArtValue::Map(ref m) => {
                            m.0.lock().unwrap_or_else(|e| e.into_inner()).len() as i64
                        }
                        ArtValue::Set(ref s) => {
                            s.0.lock().unwrap_or_else(|e| e.into_inner()).len() as i64
                        }
                        _ => {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "len: unsupported type".to_string(),
                                self.call_span,
                            ));
                            return Ok(ArtValue::none());
                        }
                    };
                    Ok(ArtValue::Int(n))
                } else {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "len: missing argument".to_string(),
                        self.call_span,
                    ));
                    Ok(ArtValue::none())
                }
            }
            core::ast::BuiltinFn::TypeOf => {
                if let Some(first) = arguments.into_iter().next() {
                    let val = self.evaluate(first)?;
                    let resolved = if let ArtValue::HeapComposite(h) = &val {
                        self.heap_objects
                            .get(&h.0)
                            .map(|o| &o.value)
                            .unwrap_or(&val)
                    } else {
                        &val
                    };
                    let t = match resolved {
                        ArtValue::Int(_) => "Int",
                        ArtValue::Float(_) => "Float",
                        ArtValue::String(_) => "String",
                        ArtValue::Bool(_) => "Bool",
                        ArtValue::Optional(_) => "Optional",
                        ArtValue::Array(_) => "Array",
                        ArtValue::Tuple(_) => "Tuple",
                        ArtValue::Map(_) => "Map",
                        ArtValue::Set(_) => "Set",
                        ArtValue::Deque(_) => "Deque",
                        ArtValue::StructInstance { .. } => "Struct",
                        ArtValue::EnumInstance { .. } => "Enum",
                        ArtValue::Function(_) => "Function",
                        ArtValue::Builtin(_) => "Builtin",
                        ArtValue::WeakRef(_) => "WeakRef",
                        ArtValue::UnownedRef(_) => "UnownedRef",
                        ArtValue::HeapComposite(_) => "Composite",
                        ArtValue::Atomic(_) => "Atomic",
                        ArtValue::Mutex(_) => "Mutex",
                        ArtValue::Actor(_) => "Actor",
                        ArtValue::Capability { .. } => "Capability",
                        ArtValue::MovedCapability => "MovedCapability",
                        ArtValue::Buffer(_) => "Buffer",
                    };
                    Ok(ArtValue::String(core::intern_arc(t)))
                } else {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "type_of: missing argument".to_string(),
                        self.call_span,
                    ));
                    Ok(ArtValue::none())
                }
            }
            core::ast::BuiltinFn::WeakNew => {
                if let Some(first) = arguments.into_iter().next() {
                    // Avalia e registra objeto
                    let val = self.evaluate(first)?;
                    let (_id, handle) = match val {
                        ArtValue::HeapComposite(h) => {
                            self.inc_heap_weak(h.0);
                            (h.0, h)
                        }
                        _other => {
                            // Para tipos escalares ainda criar wrapper heap para permitir weak.
                            let id = self.heap_register(_other);
                            self.inc_heap_weak(id);
                            (id, ObjHandle(id))
                        }
                    };
                    self.weak_created += 1;
                    Ok(ArtValue::WeakRef(handle))
                } else {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "__weak: missing arg",
                        self.call_span,
                    ));
                    Ok(ArtValue::none())
                }
            }
            core::ast::BuiltinFn::WeakGet => {
                if let Some(first) = arguments.into_iter().next() {
                    match self.evaluate(first)? {
                        ArtValue::WeakRef(h) => match self.heap_upgrade_weak(h.0) {
                            Some(v) => {
                                self.weak_upgrades += 1;
                                Ok(ArtValue::Optional(Box::new(Some(v))))
                            }
                            None => {
                                self.weak_dangling += 1;
                                Ok(ArtValue::Optional(Box::new(None)))
                            }
                        },
                        _ => {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "__weak_get: expected WeakRef",
                                self.call_span,
                            ));
                            Ok(ArtValue::none())
                        }
                    }
                } else {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "__weak_get: missing arg",
                        self.call_span,
                    ));
                    Ok(ArtValue::none())
                }
            }
            core::ast::BuiltinFn::UnownedNew => {
                if let Some(first) = arguments.into_iter().next() {
                    let val = self.evaluate(first)?;
                    let handle = match val {
                        ArtValue::HeapComposite(h) => h,
                        _other => {
                            let id = self.heap_register(_other);
                            ObjHandle(id)
                        }
                    };
                    self.unowned_created += 1;
                    Ok(ArtValue::UnownedRef(handle))
                } else {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "__unowned: missing arg",
                        self.call_span,
                    ));
                    Ok(ArtValue::none())
                }
            }
            core::ast::BuiltinFn::UnownedGet => {
                if let Some(first) = arguments.into_iter().next() {
                    match self.evaluate(first)? {
                        ArtValue::UnownedRef(h) => match self.heap_get_unowned(h.0) {
                            Some(v) => Ok(v),
                            None => {
                                self.unowned_dangling += 1;
                                self.diagnostics.push(Diagnostic::new(
                                    DiagnosticKind::Runtime,
                                    "dangling unowned reference",
                                    self.call_span,
                                ));
                                Ok(ArtValue::none())
                            }
                        },
                        _ => {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "__unowned_get: expected UnownedRef",
                                self.call_span,
                            ));
                            Ok(ArtValue::none())
                        }
                    }
                } else {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "__unowned_get: missing arg",
                        self.call_span,
                    ));
                    Ok(ArtValue::none())
                }
            }
            core::ast::BuiltinFn::OnFinalize => {
                if arguments.len() != 2 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "on_finalize espera 2 args",
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }
                let obj_val = self.evaluate(arguments[0].clone())?;
                let fn_val = self.evaluate(arguments[1].clone())?;
                let handle_opt = match obj_val {
                    ArtValue::HeapComposite(h) => Some(h),
                    _ => None,
                };
                let func_rc = match fn_val {
                    ArtValue::Function(f) => Some(f),
                    _ => None,
                };
                if let (Some(h), Some(frc)) = (handle_opt, func_rc) {
                    if let Some(o) = self.heap_objects.get(&h.0)
                        && o.alive
                    {
                        self.finalizers.insert(h.0, frc.clone());
                    }
                } else {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "on_finalize tipos inválidos",
                        self.call_span,
                    ));
                }
                Ok(ArtValue::none())
            }
            core::ast::BuiltinFn::ActorSend => {
                // Accepts actor_send(actor_id, value [, priority])
                if arguments.len() < 2 || arguments.len() > 3 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "actor_send expects 2 or 3 args".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }
                let aid_val = self.evaluate(arguments[0].clone())?;
                let mut msg_val = self.evaluate(arguments[1].clone())?;
                let priority = if arguments.len() == 3 {
                    match self.evaluate(arguments[2].clone())? {
                        ArtValue::Int(n) => n as i32,
                        _ => 0,
                    }
                } else {
                    0
                };
                // accept Actor handle variant, Int, or Optional(Actor/Int) for backward compatibility
                let aid_opt = match aid_val {
                    ArtValue::Actor(id) => Some(id),
                    ArtValue::Int(n) => Some(n as u32),
                    ArtValue::Optional(inner) => {
                        if let Some(actual) = inner.as_ref() {
                            match actual {
                                ArtValue::Actor(id) => Some(*id),
                                ArtValue::Int(n) => Some(*n as u32),
                                _ => None,
                            }
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(aid) = aid_opt {
                    // Promoção antecipada: o payload escapa para outro ator (global heap)
                    self.promote_if_escaping(None, &mut msg_val);

                    if let Some(actor) = self.actors.get_mut(&aid) {
                        let limit = actor.mailbox_limit;
                        if actor.mailbox.len() >= limit {
                            // mailbox full: signal backpressure (return false)
                            return Ok(ArtValue::Bool(false));
                        }

                        let env = core::ast::ValueEnvelope {
                            sender: self.current_actor,
                            payload: msg_val,
                            priority,
                        };
                        actor.mailbox.insert(env);
                        if actor.parked {
                            actor.parked = false;
                        }
                        return Ok(ArtValue::Bool(true));
                    } else if let Some(exec) = &mut self.executing_actor {
                        if exec.id == aid {
                            let limit = exec.mailbox_limit;
                            if exec.mailbox.len() >= limit {
                                return Ok(ArtValue::Bool(false));
                            }
                            let env = core::ast::ValueEnvelope {
                                sender: self.current_actor,
                                payload: msg_val,
                                priority,
                            };
                            exec.mailbox.insert(env);
                            if exec.parked {
                                exec.parked = false;
                            }
                            return Ok(ArtValue::Bool(true));
                        }
                    } else {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("actor_send: unknown actor id {}", aid),
                            self.call_span,
                        ));
                    }
                } else {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "actor_send: actor id must be Int".to_string(),
                        self.call_span,
                    ));
                }
                Ok(ArtValue::none())
            }
            core::ast::BuiltinFn::ActorReceive => {
                if let Some(actor) = &mut self.executing_actor {
                    if let Some(env) = actor.mailbox.pop_front() {
                        actor.parked = false;
                        return Ok(env.payload);
                    }
                    actor.parked = true;
                    return Ok(ArtValue::Optional(Box::new(None)));
                }
                if let Some(aid) = self.current_actor {
                    if let Some(actor) = self.actors.get_mut(&aid) {
                        if let Some(env) = actor.mailbox.pop_front() {
                            return Ok(env.payload);
                        } else {
                            actor.parked = true;
                            return Ok(ArtValue::Optional(Box::new(None)));
                        }
                    }
                    // If actor not found because it's currently executing and removed from map,
                    // try executing_actor
                    if let Some(exec) = &mut self.executing_actor
                        && exec.id == aid {
                            if let Some(env) = exec.mailbox.pop_front() {
                                return Ok(env.payload);
                            } else {
                                exec.parked = true;
                                return Ok(ArtValue::Optional(Box::new(None)));
                            }
                        }
                }
                self.diagnostics.push(Diagnostic::new(
                    DiagnosticKind::Runtime,
                    "actor_receive: no current actor context".to_string(),
                    self.call_span,
                ));
                Ok(ArtValue::Optional(Box::new(None)))
            }
            core::ast::BuiltinFn::ActorReceiveEnvelope => {
                // Return the full envelope (sender, payload, priority) as a StructInstance
                if let Some(aid) = self.current_actor {
                    if let Some(actor) = self.actors.get_mut(&aid) {
                        if let Some(env) = actor.mailbox.pop_front() {
                            // Build a StructInstance with fields: sender, payload, priority
                            let mut fields = std::collections::HashMap::new();
                            let sender_val = match env.sender {
                                Some(s) => ArtValue::Int(s as i64),
                                None => ArtValue::Optional(Box::new(None)),
                            };
                            fields.insert("sender".to_string(), sender_val);
                            fields.insert("payload".to_string(), env.payload);
                            fields
                                .insert("priority".to_string(), ArtValue::Int(env.priority as i64));
                            let struct_val = ArtValue::StructInstance {
                                struct_name: "Envelope".to_string(),
                                fields,
                            };
                            return Ok(struct_val);
                        } else {
                            actor.parked = true;
                            return Ok(ArtValue::Optional(Box::new(None)));
                        }
                    }
                    if let Some(exec) = &mut self.executing_actor
                        && exec.id == aid {
                            if let Some(env) = exec.mailbox.pop_front() {
                                let mut fields = std::collections::HashMap::new();
                                let sender_val = match env.sender {
                                    Some(s) => ArtValue::Int(s as i64),
                                    None => ArtValue::Optional(Box::new(None)),
                                };
                                fields.insert("sender".to_string(), sender_val);
                                fields.insert("payload".to_string(), env.payload);
                                fields.insert(
                                    "priority".to_string(),
                                    ArtValue::Int(env.priority as i64),
                                );
                                let struct_val = ArtValue::StructInstance {
                                    struct_name: "Envelope".to_string(),
                                    fields,
                                };
                                return Ok(struct_val);
                            } else {
                                exec.parked = true;
                                return Ok(ArtValue::Optional(Box::new(None)));
                            }
                        }
                }
                self.diagnostics.push(Diagnostic::new(
                    DiagnosticKind::Runtime,
                    "actor_receive_envelope: no current actor context".to_string(),
                    self.call_span,
                ));
                Ok(ArtValue::Optional(Box::new(None)))
            }
            core::ast::BuiltinFn::ActorSetMailboxLimit => {
                if arguments.len() != 2 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "actor_set_mailbox_limit expects 2 args".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }
                let aid_val = self.evaluate(arguments[0].clone())?;
                let limit_val = self.evaluate(arguments[1].clone())?;
                let aid_opt = match aid_val {
                    core::ast::ArtValue::Actor(id) => Some(id),
                    core::ast::ArtValue::Int(n) => Some(n as u32),
                    _ => None,
                };
                if let (Some(aid), core::ast::ArtValue::Int(l)) = (aid_opt, limit_val) {
                    let lim = if l < 0 { 0 } else { l as usize };
                    if let Some(actor) = self.actors.get_mut(&aid) {
                        actor.mailbox_limit = lim;
                        return Ok(ArtValue::Bool(true));
                    } else {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("actor_set_mailbox_limit: unknown actor id {}", aid),
                            self.call_span,
                        ));
                    }
                } else {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "actor_set_mailbox_limit: invalid args".to_string(),
                        self.call_span,
                    ));
                }
                Ok(ArtValue::none())
            }
            core::ast::BuiltinFn::ActorYield => {
                // actor_yield is a cooperative hint; scheduler will rotate after statement
                // For runtime, just return None; scheduler sees it's a normal statement boundary.
                Ok(ArtValue::none())
            }
            core::ast::BuiltinFn::EnvelopeNew => {
                // envelope(sender, payload, priority)
                if arguments.len() != 3 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "envelope expects 3 args".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }
                let sender_val = self.evaluate(arguments[0].clone())?;
                let payload_val = self.evaluate(arguments[1].clone())?;
                let priority_val = self.evaluate(arguments[2].clone())?;
                let sender_field = match sender_val {
                    ArtValue::Optional(boxed) => match *boxed {
                        Some(ArtValue::Int(n)) => ArtValue::Int(n),
                        _ => ArtValue::Optional(Box::new(None)),
                    },
                    ArtValue::Int(n) => ArtValue::Int(n),
                    other => other,
                };
                let priority = if let ArtValue::Int(n) = priority_val {
                    n as i32
                } else {
                    0
                };
                let mut fields = std::collections::HashMap::new();
                fields.insert("sender".to_string(), sender_field);
                fields.insert("payload".to_string(), payload_val);
                fields.insert("priority".to_string(), ArtValue::Int(priority as i64));
                let struct_val = ArtValue::StructInstance {
                    struct_name: "Envelope".to_string(),
                    fields,
                };
                Ok(self.heapify_composite(struct_val))
            }
            core::ast::BuiltinFn::MakeEnvelope => {
                // make_envelope(payload [, priority]) -> Envelope with sender=current_actor
                if arguments.is_empty() || arguments.len() > 2 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "make_envelope expects 1 or 2 args".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }
                let payload_val = self.evaluate(arguments[0].clone())?;
                let priority = if arguments.len() == 2 {
                    match self.evaluate(arguments[1].clone())? {
                        ArtValue::Int(n) => n as i32,
                        _ => 0,
                    }
                } else {
                    0
                };
                let sender_field = if let Some(sid) = self.current_actor {
                    ArtValue::Int(sid as i64)
                } else {
                    ArtValue::Optional(Box::new(None))
                };
                let mut fields = std::collections::HashMap::new();
                fields.insert("sender".to_string(), sender_field);
                fields.insert("payload".to_string(), payload_val);
                fields.insert("priority".to_string(), ArtValue::Int(priority as i64));
                let struct_val = ArtValue::StructInstance {
                    struct_name: "Envelope".to_string(),
                    fields,
                };
                Ok(self.heapify_composite(struct_val))
            }
            core::ast::BuiltinFn::RunActors => {
                // run_actors([max_steps]) -> drive scheduler until idle or up to max_steps
                let max_steps = if arguments.len() == 1 {
                    match self.evaluate(arguments[0].clone())? {
                        ArtValue::Int(n) if n >= 0 => n as usize,
                        _other => {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "run_actors: invalid max_steps argument".to_string(),
                                self.call_span,
                            ));
                            return Ok(ArtValue::none());
                        }
                    }
                } else {
                    usize::MAX
                };
                self.run_actors_round_robin(max_steps);
                Ok(ArtValue::none())
            }
            // Prototype atomic/mutex builtins for performant blocks (single-threaded semantics)
            core::ast::BuiltinFn::AtomicNew => {
                if arguments.len() != 1 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "atomic_new expects 1 arg".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }
                let val = self.evaluate(arguments[0].clone())?;
                Ok(self.heap_create_atomic(val))
            }
            core::ast::BuiltinFn::AtomicLoad => {
                if arguments.len() != 1 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "atomic_load expects 1 arg".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }
                let a = self.evaluate(arguments[0].clone())?;
                if let ArtValue::Atomic(h) = a {
                    return Ok(self.heap_atomic_load(h).unwrap_or(ArtValue::none()));
                }
                Ok(ArtValue::none())
            }
            core::ast::BuiltinFn::AtomicStore => {
                if arguments.len() != 2 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "atomic_store expects 2 args".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }
                let a = self.evaluate(arguments[0].clone())?;
                let v = self.evaluate(arguments[1].clone())?;
                if let ArtValue::Atomic(h) = a {
                    return Ok(ArtValue::Bool(self.heap_atomic_store(h, v)));
                }
                Ok(ArtValue::Bool(false))
            }
            core::ast::BuiltinFn::AtomicAdd => {
                if arguments.len() != 2 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "atomic_add expects 2 args".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }
                let a = self.evaluate(arguments[0].clone())?;
                let delta = self.evaluate(arguments[1].clone())?;
                if let (ArtValue::Atomic(h), ArtValue::Int(d)) = (a, delta)
                    && let Some(new) = self.heap_atomic_add(h, d) {
                        return Ok(ArtValue::Int(new));
                    }
                Ok(ArtValue::none())
            }
            core::ast::BuiltinFn::MutexNew => {
                if arguments.len() != 1 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "mutex_new expects 1 arg".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }
                let v = self.evaluate(arguments[0].clone())?;
                Ok(self.heap_create_mutex(v))
            }
            core::ast::BuiltinFn::MutexLock => {
                if arguments.len() != 1 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "mutex_lock expects 1 arg".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }
                let a = self.evaluate(arguments[0].clone())?;
                if let ArtValue::Mutex(h) = a {
                    return Ok(ArtValue::Bool(self.heap_mutex_lock(h)));
                }
                Ok(ArtValue::Bool(false))
            }
            core::ast::BuiltinFn::MutexUnlock => {
                if arguments.len() != 1 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "mutex_unlock expects 1 arg".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }
                let a = self.evaluate(arguments[0].clone())?;
                if let ArtValue::Mutex(h) = a {
                    return Ok(ArtValue::Bool(self.heap_mutex_unlock(h)));
                }
                Ok(ArtValue::Bool(false))
            }
            core::ast::BuiltinFn::ArenaNew => {
                if !arguments.is_empty() {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "arena_new expects no arguments".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }
                let arena_id = self.next_arena_id;
                self.next_arena_id = self.next_arena_id.saturating_add(1);
                self.reusable_arenas.insert(arena_id);
                Ok(ArtValue::Int(arena_id as i64))
            }
            core::ast::BuiltinFn::ArenaRelease => {
                if arguments.len() != 1 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "arena_release expects exactly one arena id argument".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::Bool(false));
                }

                let arena_val = self.evaluate(arguments[0].clone())?;
                let arena_id = match arena_val {
                    ArtValue::Int(i) if i > 0 && i <= u32::MAX as i64 => i as u32,
                    _ => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "arena_release expects a positive Int arena id".to_string(),
                            self.call_span,
                        ));
                        return Ok(ArtValue::Bool(false));
                    }
                };

                if !self.reusable_arenas.contains(&arena_id) {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        format!("arena_release: unknown reusable arena id {}", arena_id),
                        self.call_span,
                    ));
                    return Ok(ArtValue::Bool(false));
                }

                self.finalize_arena(arena_id);
                Ok(ArtValue::Bool(true))
            }
            core::ast::BuiltinFn::ArenaWith => {
                if arguments.len() != 2 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "arena_with expects (arena_id, callback)".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }

                let arena_val = self.evaluate(arguments[0].clone())?;
                let arena_id = match arena_val {
                    ArtValue::Int(i) if i > 0 && i <= u32::MAX as i64 => i as u32,
                    _ => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "arena_with expects a positive Int arena id as first argument"
                                .to_string(),
                            self.call_span,
                        ));
                        return Ok(ArtValue::none());
                    }
                };

                if !self.reusable_arenas.contains(&arena_id) {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        format!("arena_with: unknown reusable arena id {}", arena_id),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }

                let callback = self.evaluate(arguments[1].clone())?;
                let previous_arena = self.current_arena;
                let previous_arena_with_active = self.arena_with_active;
                self.arena_with_active = true;
                self.current_arena = Some(arena_id);

                let callback_result = match callback {
                    ArtValue::Function(func) => self.call_function(func, None, Vec::new()),
                    ArtValue::Builtin(bi) => self.call_builtin(bi, Vec::new()),
                    other => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!(
                                "arena_with expects callback as function/builtin, got {:?}",
                                other
                            ),
                            self.call_span,
                        ));
                        Ok(ArtValue::none())
                    }
                };

                self.finalize_arena(arena_id);
                self.current_arena = previous_arena;
                self.arena_with_active = previous_arena_with_active;
                callback_result
            }
            core::ast::BuiltinFn::IdlSchema => {
                if arguments.len() != 1 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "idl_schema expects exactly one struct name argument".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }

                let schema_name = self.evaluate(arguments[0].clone())?;
                let struct_name = match schema_name {
                    ArtValue::String(s) => s.to_string(),
                    _ => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "idl_schema expects struct name as String".to_string(),
                            self.call_span,
                        ));
                        return Ok(ArtValue::none());
                    }
                };

                match self.build_idl_schema_map(&struct_name) {
                    Some(schema) => Ok(schema),
                    None => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("idl_schema: unknown struct '{}'", struct_name),
                            self.call_span,
                        ));
                        Ok(ArtValue::none())
                    }
                }
            }
            core::ast::BuiltinFn::IdlValidate => {
                if arguments.len() != 2 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "idl_validate expects (value, struct_name)".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::Bool(false));
                }

                let value = self.evaluate(arguments[0].clone())?;
                let schema_name = self.evaluate(arguments[1].clone())?;
                let struct_name = match schema_name {
                    ArtValue::String(s) => s.to_string(),
                    _ => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "idl_validate expects schema name as String".to_string(),
                            self.call_span,
                        ));
                        return Ok(ArtValue::Bool(false));
                    }
                };

                let Some(struct_def) = self.type_registry.get_struct(&struct_name).cloned() else {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        format!("idl_validate: unknown struct '{}'", struct_name),
                        self.call_span,
                    ));
                    return Ok(ArtValue::Bool(false));
                };

                let resolved = self.resolve_composite(&value).clone();
                let ArtValue::StructInstance {
                    struct_name: value_struct_name,
                    fields,
                } = resolved
                else {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        format!(
                            "idl_validate: expected Struct '{}' message, got {}",
                            struct_name,
                            self.runtime_type_label(&value)
                        ),
                        self.call_span,
                    ));
                    return Ok(ArtValue::Bool(false));
                };

                if value_struct_name != struct_name {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        format!(
                            "idl_validate: expected struct '{}' but got '{}'",
                            struct_name, value_struct_name
                        ),
                        self.call_span,
                    ));
                    return Ok(ArtValue::Bool(false));
                }

                for (field_name, expected_ty) in &struct_def.fields {
                    let Some(field_val) = fields.get(field_name) else {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!(
                                "idl_validate: missing field '{}' for schema '{}'",
                                field_name, struct_name
                            ),
                            self.call_span,
                        ));
                        return Ok(ArtValue::Bool(false));
                    };

                    if !self.value_matches_declared_type(field_val, expected_ty) {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!(
                                "idl_validate: field '{}' expected '{}' but got '{}'",
                                field_name,
                                expected_ty,
                                self.runtime_type_label(field_val)
                            ),
                            self.call_span,
                        ));
                        return Ok(ArtValue::Bool(false));
                    }
                }

                Ok(ArtValue::Bool(true))
            }
            core::ast::BuiltinFn::BufferNew => {
                if arguments.len() != 1 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "buffer_new expects exactly one integer size argument".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }
                match self.evaluate(arguments[0].clone())? {
                    ArtValue::Int(size) if size >= 0 => {
                        let buf = vec![0u8; size as usize];
                        Ok(ArtValue::Buffer(buf.into()))
                    }
                    _ => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "buffer_new argument must be a non-negative Int".to_string(),
                            self.call_span,
                        ));
                        Ok(ArtValue::none())
                    }
                }
            }
            core::ast::BuiltinFn::Serialize => {
                if arguments.len() != 1 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "serialize expects exactly one argument".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }
                let val = self.evaluate(arguments[0].clone())?;
                let normalized = self.normalize_for_serialization(&val);
                let mut out = Vec::new();
                match crate::interpreter::encode_val(&normalized, &mut out) {
                    Ok(_) => Ok(ArtValue::Buffer(out.into())),
                    Err(e) => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("serialize error: {}", e),
                            self.call_span,
                        ));
                        Ok(ArtValue::none())
                    }
                }
            }
            core::ast::BuiltinFn::Deserialize => {
                if arguments.len() != 1 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "deserialize expects exactly one Buffer argument".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }
                match self.evaluate(arguments[0].clone())? {
                    ArtValue::Buffer(buf) => {
                        let mut cur = std::io::Cursor::new(buf.as_ref());
                        match crate::interpreter::decode_val(&mut cur) {
                            Ok(val) => Ok(val),
                            Err(e) => {
                                self.diagnostics.push(Diagnostic::new(
                                    DiagnosticKind::Runtime,
                                    format!("deserialize error: {}", e),
                                    self.call_span,
                                ));
                                Ok(ArtValue::none())
                            }
                        }
                    }
                    _ => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "deserialize requires a Buffer".to_string(),
                            self.call_span,
                        ));
                        Ok(ArtValue::none())
                    }
                }
            }
            core::ast::BuiltinFn::CapabilityAcquire => {
                if arguments.len() != 1 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "capability_acquire expects exactly one capability kind argument"
                            .to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }

                let kind = self.evaluate(arguments[0].clone())?;
                let kind_name = match kind {
                    ArtValue::String(s) => s,
                    _ => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "capability_acquire expects capability kind as String".to_string(),
                            self.call_span,
                        ));
                        return Ok(ArtValue::none());
                    }
                };

                let id = self.next_capability_id;
                self.next_capability_id = self.next_capability_id.saturating_add(1);
                Ok(ArtValue::Capability {
                    kind: kind_name,
                    id,
                })
            }
            core::ast::BuiltinFn::CapabilityKind => {
                if arguments.len() != 1 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "capability_kind expects exactly one capability argument".to_string(),
                        self.call_span,
                    ));
                    return Ok(ArtValue::none());
                }

                let cap = self.evaluate(arguments[0].clone())?;
                match cap {
                    ArtValue::Capability { kind, .. } => Ok(ArtValue::String(kind)),
                    _ => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "capability_kind expects a Capability token".to_string(),
                            self.call_span,
                        ));
                        Ok(ArtValue::none())
                    }
                }
            }

            // ── String builtins ──────────────────────────────────────────────

            core::ast::BuiltinFn::StrSplit => {
                let mut args = arguments.into_iter();
                match (args.next(), args.next()) {
                    (Some(s_expr), Some(sep_expr)) => {
                        match (self.evaluate(s_expr)?, self.evaluate(sep_expr)?) {
                            (ArtValue::String(s), ArtValue::String(sep)) => {
                                let parts = s
                                    .split(sep.as_ref())
                                    .map(|p| ArtValue::String(Arc::from(p)))
                                    .collect();
                                Ok(ArtValue::Array(parts))
                            }
                            _ => {
                                self.diagnostics.push(Diagnostic::new(
                                    DiagnosticKind::Runtime,
                                    "str_split expects (String, String)".to_string(),
                                    self.call_span,
                                ));
                                Ok(ArtValue::none())
                            }
                        }
                    }
                    _ => Ok(ArtValue::none()),
                }
            }

            core::ast::BuiltinFn::StrJoin => {
                let mut args = arguments.into_iter();
                match (args.next(), args.next()) {
                    (Some(arr_expr), Some(sep_expr)) => {
                        let arr_val = self.evaluate(arr_expr)?;
                        let arr_val = self.resolve_composite(&arr_val).clone();
                        match (arr_val, self.evaluate(sep_expr)?) {
                            (ArtValue::Array(arr), ArtValue::String(sep)) => {
                                let parts: Vec<String> =
                                    arr.iter().map(|v| v.to_string()).collect();
                                Ok(ArtValue::String(Arc::from(parts.join(sep.as_ref()))))
                            }
                            _ => {
                                self.diagnostics.push(Diagnostic::new(
                                    DiagnosticKind::Runtime,
                                    "str_join expects (Array, String)".to_string(),
                                    self.call_span,
                                ));
                                Ok(ArtValue::none())
                            }
                        }
                    }
                    _ => Ok(ArtValue::none()),
                }
            }

            core::ast::BuiltinFn::StrContains => {
                let mut args = arguments.into_iter();
                match (args.next(), args.next()) {
                    (Some(s_expr), Some(sub_expr)) => {
                        match (self.evaluate(s_expr)?, self.evaluate(sub_expr)?) {
                            (ArtValue::String(s), ArtValue::String(sub)) => {
                                Ok(ArtValue::Bool(s.contains(sub.as_ref())))
                            }
                            _ => {
                                self.diagnostics.push(Diagnostic::new(
                                    DiagnosticKind::Runtime,
                                    "str_contains expects (String, String)".to_string(),
                                    self.call_span,
                                ));
                                Ok(ArtValue::none())
                            }
                        }
                    }
                    _ => Ok(ArtValue::none()),
                }
            }

            core::ast::BuiltinFn::StrStartsWith => {
                let mut args = arguments.into_iter();
                match (args.next(), args.next()) {
                    (Some(s_expr), Some(prefix_expr)) => {
                        match (self.evaluate(s_expr)?, self.evaluate(prefix_expr)?) {
                            (ArtValue::String(s), ArtValue::String(prefix)) => {
                                Ok(ArtValue::Bool(s.starts_with(prefix.as_ref())))
                            }
                            _ => {
                                self.diagnostics.push(Diagnostic::new(
                                    DiagnosticKind::Runtime,
                                    "str_starts_with expects (String, String)".to_string(),
                                    self.call_span,
                                ));
                                Ok(ArtValue::none())
                            }
                        }
                    }
                    _ => Ok(ArtValue::none()),
                }
            }

            core::ast::BuiltinFn::StrReplace => {
                let mut args = arguments.into_iter();
                match (args.next(), args.next(), args.next()) {
                    (Some(s_expr), Some(from_expr), Some(to_expr)) => {
                        match (
                            self.evaluate(s_expr)?,
                            self.evaluate(from_expr)?,
                            self.evaluate(to_expr)?,
                        ) {
                            (
                                ArtValue::String(s),
                                ArtValue::String(from),
                                ArtValue::String(to),
                            ) => Ok(ArtValue::String(Arc::from(
                                s.replace(from.as_ref(), to.as_ref()),
                            ))),
                            _ => {
                                self.diagnostics.push(Diagnostic::new(
                                    DiagnosticKind::Runtime,
                                    "str_replace expects (String, String, String)".to_string(),
                                    self.call_span,
                                ));
                                Ok(ArtValue::none())
                            }
                        }
                    }
                    _ => Ok(ArtValue::none()),
                }
            }

            core::ast::BuiltinFn::StrSlice => {
                let mut args = arguments.into_iter();
                match (args.next(), args.next(), args.next()) {
                    (Some(s_expr), Some(start_expr), Some(end_expr)) => {
                        match (
                            self.evaluate(s_expr)?,
                            self.evaluate(start_expr)?,
                            self.evaluate(end_expr)?,
                        ) {
                            (ArtValue::String(s), ArtValue::Int(start), ArtValue::Int(end)) => {
                                let chars: Vec<char> = s.chars().collect();
                                let len = chars.len() as i64;
                                let start =
                                    if start < 0 { (len + start).max(0) } else { start.min(len) }
                                        as usize;
                                let end =
                                    if end < 0 { (len + end).max(0) } else { end.min(len) }
                                        as usize;
                                let end = end.max(start);
                                let result: String = chars[start..end].iter().collect();
                                Ok(ArtValue::String(Arc::from(result)))
                            }
                            _ => {
                                self.diagnostics.push(Diagnostic::new(
                                    DiagnosticKind::Runtime,
                                    "str_slice expects (String, Int, Int)".to_string(),
                                    self.call_span,
                                ));
                                Ok(ArtValue::none())
                            }
                        }
                    }
                    _ => Ok(ArtValue::none()),
                }
            }

            core::ast::BuiltinFn::StrToInt => {
                if let Some(s_expr) = arguments.into_iter().next() {
                    match self.evaluate(s_expr)? {
                        ArtValue::String(s) => match s.trim().parse::<i64>() {
                            Ok(n) => Ok(ArtValue::EnumInstance {
                                enum_name: "Result".to_string(),
                                variant: "Ok".to_string(),
                                values: vec![ArtValue::Int(n)],
                            }),
                            Err(e) => Ok(ArtValue::EnumInstance {
                                enum_name: "Result".to_string(),
                                variant: "Err".to_string(),
                                values: vec![ArtValue::String(Arc::from(e.to_string()))],
                            }),
                        },
                        _ => {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "str_to_int expects a String".to_string(),
                                self.call_span,
                            ));
                            Ok(ArtValue::none())
                        }
                    }
                } else {
                    Ok(ArtValue::none())
                }
            }

            core::ast::BuiltinFn::StrToFloat => {
                if let Some(s_expr) = arguments.into_iter().next() {
                    match self.evaluate(s_expr)? {
                        ArtValue::String(s) => match s.trim().parse::<f64>() {
                            Ok(n) => Ok(ArtValue::EnumInstance {
                                enum_name: "Result".to_string(),
                                variant: "Ok".to_string(),
                                values: vec![ArtValue::Float(n)],
                            }),
                            Err(e) => Ok(ArtValue::EnumInstance {
                                enum_name: "Result".to_string(),
                                variant: "Err".to_string(),
                                values: vec![ArtValue::String(Arc::from(e.to_string()))],
                            }),
                        },
                        _ => {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "str_to_float expects a String".to_string(),
                                self.call_span,
                            ));
                            Ok(ArtValue::none())
                        }
                    }
                } else {
                    Ok(ArtValue::none())
                }
            }
            core::ast::BuiltinFn::DequeNew => Ok(ArtValue::Deque(core::ast::DequeRef(
                std::sync::Arc::new(std::sync::Mutex::new(std::collections::VecDeque::new())),
            ))),
            core::ast::BuiltinFn::DequePushFront => {
                let mut args = arguments.into_iter();
                if let (Some(deque_expr), Some(val_expr)) = (args.next(), args.next()) {
                    let deque_val = self.evaluate(deque_expr)?;
                    let v = self.evaluate(val_expr)?;
                    if let ArtValue::Deque(d) = deque_val {
                        d.0.lock().unwrap_or_else(|e| e.into_inner()).push_front(v);
                    }
                }
                Ok(ArtValue::none())
            }
            core::ast::BuiltinFn::DequePushBack => {
                let mut args = arguments.into_iter();
                if let (Some(deque_expr), Some(val_expr)) = (args.next(), args.next()) {
                    let deque_val = self.evaluate(deque_expr)?;
                    let v = self.evaluate(val_expr)?;
                    if let ArtValue::Deque(d) = deque_val {
                        d.0.lock().unwrap_or_else(|e| e.into_inner()).push_back(v);
                    }
                }
                Ok(ArtValue::none())
            }
            core::ast::BuiltinFn::DequePopFront => {
                let mut args = arguments.into_iter();
                if let Some(deque_expr) = args.next() {
                    let deque_val = self.evaluate(deque_expr)?;
                    if let ArtValue::Deque(d) = deque_val {
                        match d.0.lock().unwrap_or_else(|e| e.into_inner()).pop_front() {
                            Some(v) => return Ok(ArtValue::Optional(Box::new(Some(v)))),
                            None => return Ok(ArtValue::none()),
                        }
                    }
                }
                Ok(ArtValue::none())
            }
            core::ast::BuiltinFn::DequePopBack => {
                let mut args = arguments.into_iter();
                if let Some(deque_expr) = args.next() {
                    let deque_val = self.evaluate(deque_expr)?;
                    if let ArtValue::Deque(d) = deque_val {
                        match d.0.lock().unwrap_or_else(|e| e.into_inner()).pop_back() {
                            Some(v) => return Ok(ArtValue::Optional(Box::new(Some(v)))),
                            None => return Ok(ArtValue::none()),
                        }
                    }
                }
                Ok(ArtValue::none())
            }
            core::ast::BuiltinFn::DequeLen => {
                let mut args = arguments.into_iter();
                if let Some(deque_expr) = args.next() {
                    let deque_val = self.evaluate(deque_expr)?;
                    if let ArtValue::Deque(d) = deque_val {
                        let len = d.0.lock().unwrap_or_else(|e| e.into_inner()).len();
                        return Ok(ArtValue::Int(len as i64));
                    }
                }
                Ok(ArtValue::Int(0))
            }
        }
    }
}
