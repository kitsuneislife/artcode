mod sourcemap;
pub use sourcemap::SourceMapBuilder;

use core::ast::{ArtValue, Expr, InterpolatedPart, MatchPattern, Stmt, TemplateAttrValue, TemplateNode};

pub struct CodegenOptions {
    pub source_file: Option<String>,
    pub emit_source_map: bool,
    pub module_format: ModuleFormat,
}

#[derive(Default, PartialEq)]
pub enum ModuleFormat {
    #[default]
    Esm,
    Iife,
    /// Bundle mode: import statements are suppressed (bundler inlines deps externally).
    Bundle,
}

impl Default for CodegenOptions {
    fn default() -> Self {
        Self {
            source_file: None,
            emit_source_map: false,
            module_format: ModuleFormat::Esm,
        }
    }
}

pub struct JsOutput {
    pub code: String,
    pub source_map: Option<String>,
}

pub struct CodegenJs {
    output: String,
    indent: usize,
    gen_line: u32,
    gen_col: u32,
    source_map: SourceMapBuilder,
    options: CodegenOptions,
}

impl CodegenJs {
    pub fn new(options: CodegenOptions) -> Self {
        Self {
            output: String::new(),
            indent: 0,
            gen_line: 0,
            gen_col: 0,
            source_map: SourceMapBuilder::new(),
            options,
        }
    }

    pub fn emit_program(mut self, program: &[Stmt]) -> JsOutput {
        for stmt in program {
            self.emit_stmt(stmt);
        }
        let source_map = if self.options.emit_source_map {
            let name = self.options.source_file.as_deref().unwrap_or("input.art");
            Some(self.source_map.build(name, None))
        } else {
            None
        };
        JsOutput {
            code: self.output,
            source_map,
        }
    }

    // ── internal helpers ─────────────────────────────────────────────────────

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
        self.gen_col += s.len() as u32;
    }

    fn newline(&mut self) {
        self.output.push('\n');
        self.gen_line += 1;
        self.gen_col = 0;
    }

    fn indent_str(&self) -> String {
        "    ".repeat(self.indent)
    }

    #[allow(dead_code)]
    fn write_indent(&mut self) {
        let s = self.indent_str();
        self.write(&s);
    }

    fn record(&mut self, src_line: usize, src_col: usize) {
        if self.options.emit_source_map {
            self.source_map.add(
                self.gen_line,
                self.gen_col,
                src_line.saturating_sub(1) as u32,
                src_col as u32,
            );
        }
    }

    fn js_ident(name: &str) -> String {
        // Map reserved JS words that collide with Artcode identifiers
        match name {
            "delete" => "__delete".to_string(),
            "class" | "new" | "typeof" | "instanceof" | "void" | "in" | "of" | "yield"
            | "await" | "async" | "static" | "super" | "extends" | "export" | "import"
            | "default" | "from" | "as" | "with" | "debugger" | "implements" | "interface"
            | "package" | "private" | "protected" | "public" => {
                format!("__art_{}", name)
            }
            other => other.to_string(),
        }
    }

    fn escape_string(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c => out.push(c),
            }
        }
        out
    }

    // ── statements ───────────────────────────────────────────────────────────

    fn emit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Expression(expr) => {
                let ind = self.indent_str();
                self.write(&ind);
                let js = self.emit_expr(expr);
                self.write(&js);
                self.write(";");
                self.newline();
            }

            Stmt::Let { pattern, initializer, .. } => {
                let ind = self.indent_str();
                self.write(&ind);
                match pattern {
                    MatchPattern::Variable(tok) | MatchPattern::Binding(tok) => {
                        self.record(tok.line, tok.col);
                        let name = Self::js_ident(&tok.lexeme);
                        let rhs = self.emit_expr(initializer);
                        self.write(&format!("const {} = {};", name, rhs));
                    }
                    MatchPattern::Tuple(pats) => {
                        let names: Vec<String> = pats
                            .iter()
                            .map(|p| match p {
                                MatchPattern::Variable(t) | MatchPattern::Binding(t) => {
                                    Self::js_ident(&t.lexeme)
                                }
                                _ => "_".to_string(),
                            })
                            .collect();
                        let rhs = self.emit_expr(initializer);
                        self.write(&format!("const [{}] = {};", names.join(", "), rhs));
                    }
                    _ => {
                        let rhs = self.emit_expr(initializer);
                        self.write(&format!("const _ = {};", rhs));
                    }
                }
                self.newline();
            }

            Stmt::Block { statements } => {
                let ind = self.indent_str();
                self.write(&ind);
                self.write("{");
                self.newline();
                self.indent += 1;
                for s in statements {
                    self.emit_stmt(s);
                }
                self.indent -= 1;
                let ind = self.indent_str();
                self.write(&ind);
                self.write("}");
                self.newline();
            }

            Stmt::If { condition, then_branch, else_branch } => {
                let ind = self.indent_str();
                self.write(&ind);
                let cond = self.emit_expr(condition);
                self.write(&format!("if ({}) ", cond));
                self.emit_stmt_inline(then_branch);
                if let Some(else_b) = else_branch {
                    self.write(" else ");
                    self.emit_stmt_inline(else_b);
                }
                self.newline();
            }

            Stmt::IfLet { pattern, value, then_branch, else_branch } => {
                let ind = self.indent_str();
                self.write(&ind);
                let val = self.emit_expr(value);
                let tmp = "__iflet_val";
                self.write(&format!("{{ const {} = {}; ", tmp, val));
                let cond = self.emit_iflet_condition(pattern, tmp);
                self.write(&format!("if ({}) ", cond));
                let bindings = self.emit_iflet_bindings(pattern, tmp);
                self.write("{");
                self.newline();
                self.indent += 1;
                if !bindings.is_empty() {
                    let ind2 = self.indent_str();
                    self.write(&ind2);
                    self.write(&bindings);
                    self.newline();
                }
                self.emit_stmt(then_branch);
                self.indent -= 1;
                let ind2 = self.indent_str();
                self.write(&ind2);
                self.write("}");
                if let Some(else_b) = else_branch {
                    self.write(" else ");
                    self.emit_stmt_inline(else_b);
                }
                self.write(" }");
                self.newline();
            }

            Stmt::TryCatch { try_branch, catch_name, catch_branch } => {
                let ind = self.indent_str();
                self.write(&ind);
                self.write("try ");
                self.emit_stmt_inline(try_branch);
                let cn = Self::js_ident(&catch_name.lexeme);
                self.write(&format!(" catch ({}) ", cn));
                self.emit_stmt_inline(catch_branch);
                self.newline();
            }

            Stmt::Function { name, params, body, method_owner, type_params: _, is_async, .. } => {
                let ind = self.indent_str();
                self.write(&ind);
                self.record(name.line, name.col);
                let fname = Self::js_ident(&name.lexeme);
                let pnames: Vec<String> =
                    params.iter().map(|p| Self::js_ident(&p.name.lexeme)).collect();
                let async_kw = if *is_async { "async " } else { "" };
                if let Some(owner) = method_owner {
                    let owner_js = Self::js_ident(owner);
                    self.write(&format!(
                        "{}{}_{}.prototype.{} = function({}) ",
                        async_kw, owner_js, owner_js, fname, pnames.join(", ")
                    ));
                } else {
                    self.write(&format!(
                        "{}function {}({}) ",
                        async_kw, fname, pnames.join(", ")
                    ));
                }
                self.emit_stmt_inline(body);
                self.newline();
            }

            Stmt::While { condition, body } => {
                let ind = self.indent_str();
                self.write(&ind);
                let cond = self.emit_expr(condition);
                self.write(&format!("while ({}) ", cond));
                self.emit_stmt_inline(body);
                self.newline();
            }

            Stmt::For { element, iterator, body } => {
                let ind = self.indent_str();
                self.write(&ind);
                let elem = Self::js_ident(&element.lexeme);
                let iter = self.emit_expr(iterator);
                self.write(&format!("for (const {} of {}) ", elem, iter));
                self.emit_stmt_inline(body);
                self.newline();
            }

            Stmt::Return { value } => {
                let ind = self.indent_str();
                self.write(&ind);
                match value {
                    Some(v) => {
                        let js = self.emit_expr(v);
                        self.write(&format!("return {};", js));
                    }
                    None => self.write("return;"),
                }
                self.newline();
            }

            Stmt::StructDecl { name, fields } => {
                let ind = self.indent_str();
                self.write(&ind);
                self.record(name.line, name.col);
                let cname = &name.lexeme;
                let field_names: Vec<String> =
                    fields.iter().map(|(f, _)| Self::js_ident(&f.lexeme)).collect();
                self.write(&format!("class {} {{\n", cname));
                self.indent += 1;
                let ind2 = self.indent_str();
                self.write(&format!(
                    "{}constructor({}) {{\n",
                    ind2,
                    field_names.join(", ")
                ));
                self.indent += 1;
                for fname in &field_names {
                    let ind3 = self.indent_str();
                    self.write(&format!("{}this.{} = {};\n", ind3, fname, fname));
                }
                self.indent -= 1;
                let ind2 = self.indent_str();
                self.write(&format!("{}}}\n", ind2));
                self.indent -= 1;
                let ind = self.indent_str();
                self.write(&format!("{}}}", ind));
                self.newline();
            }

            Stmt::EnumDecl { name, variants } => {
                let ind = self.indent_str();
                self.record(name.line, name.col);
                let ename = &name.lexeme;
                self.write(&format!("{}const {} = {{\n", ind, ename));
                self.indent += 1;
                for (variant, payload_types) in variants {
                    let vname = &variant.lexeme;
                    let ind2 = self.indent_str();
                    if let Some(types) = payload_types {
                        let args: Vec<String> = (0..types.len())
                            .map(|i| format!("_v{}", i))
                            .collect();
                        self.write(&format!(
                            "{}{}(...[{}]) {{ return {{ tag: \"{}\", payload: [{}] }}; }},\n",
                            ind2,
                            vname,
                            args.join(", "),
                            vname,
                            args.join(", ")
                        ));
                    } else {
                        self.write(&format!(
                            "{}{}() {{ return {{ tag: \"{}\" }}; }},\n",
                            ind2, vname, vname
                        ));
                    }
                }
                self.indent -= 1;
                let ind = self.indent_str();
                self.write(&format!("{}}};", ind));
                self.newline();
            }

            Stmt::Match { expr, cases } => {
                self.emit_match(expr, cases);
            }

            Stmt::ImplBlock { type_name, methods } => {
                for method in methods {
                    if let Stmt::Function { name, params, body, is_async, .. } = method {
                        let ind = self.indent_str();
                        self.write(&ind);
                        let tname = Self::js_ident(type_name);
                        let mname = Self::js_ident(&name.lexeme);
                        let pnames: Vec<String> = params
                            .iter()
                            .filter(|p| p.name.lexeme != "self")
                            .map(|p| Self::js_ident(&p.name.lexeme))
                            .collect();
                        let async_kw = if *is_async { "async " } else { "" };
                        self.write(&format!(
                            "{}.prototype.{} = {}function({}) ",
                            tname, mname, async_kw, pnames.join(", ")
                        ));
                        self.emit_stmt_inline(body);
                        self.newline();
                    }
                }
            }

            Stmt::Performant { statements } => {
                let ind = self.indent_str();
                self.write(&ind);
                self.write("(() => {");
                self.newline();
                self.indent += 1;
                for s in statements {
                    self.emit_stmt(s);
                }
                self.indent -= 1;
                let ind = self.indent_str();
                self.write(&ind);
                self.write("})();");
                self.newline();
            }

            Stmt::SpawnActor { body } => {
                // Actors → Web Workers. The body is serialized as a blob URL.
                let ind = self.indent_str();
                self.write(&ind);
                self.write("/* spawn actor → Worker */\n");
                self.write(&ind);
                self.write("const __actor_src = `");
                self.indent += 1;
                let mut inner = CodegenJs::new(CodegenOptions {
                    emit_source_map: false,
                    ..Default::default()
                });
                for s in body {
                    inner.emit_stmt(s);
                }
                self.write(&inner.output.replace('`', "\\`"));
                self.indent -= 1;
                self.write("`;\n");
                self.write(&ind);
                self.write("const __actor_blob = new Blob([__actor_src], { type: 'application/javascript' });\n");
                self.write(&ind);
                self.write("const __actor_worker = new Worker(URL.createObjectURL(__actor_blob));");
                self.newline();
            }

            Stmt::Import { path } => {
                // Bundle mode: imports are inlined by the bundler; suppress the statement.
                if self.options.module_format == ModuleFormat::Bundle {
                    return;
                }
                let ind = self.indent_str();
                self.write(&ind);
                let parts: Vec<String> = path.iter().map(|t| t.lexeme.clone()).collect();
                let module_path = format!("./{}.js", parts.join("/"));
                let symbol = Self::js_ident(&parts.last().cloned().unwrap_or_default());
                self.write(&format!("import * as {} from \"{}\";", symbol, module_path));
                self.newline();
            }

            Stmt::ShellCommand { .. } => {
                let ind = self.indent_str();
                self.write(&ind);
                self.write("/* shell command: not supported in JS target */");
                self.newline();
            }

            Stmt::ComponentBlock { name, bindings, view } => {
                self.emit_component(name, bindings, view);
            }

            Stmt::QualifiedBinding { qualifier, name, value, .. } => {
                use core::ast::BindingQualifier;
                let ind = self.indent_str();
                self.write(&ind);
                match qualifier {
                    BindingQualifier::State => {
                        let val = value.as_deref().map(|v| self.emit_expr(v)).unwrap_or_else(|| "undefined".to_string());
                        self.write(&format!("let {} = {};", Self::js_ident(&name.lexeme), val));
                    }
                    BindingQualifier::Prop => {
                        self.write(&format!("/* prop {} */", Self::js_ident(&name.lexeme)));
                    }
                    BindingQualifier::Memo => {
                        let val = value.as_deref().map(|v| self.emit_expr(v)).unwrap_or_else(|| "undefined".to_string());
                        self.write(&format!("let {} = {};", Self::js_ident(&name.lexeme), val));
                    }
                    BindingQualifier::Ref => {
                        let val = value.as_deref().map(|v| self.emit_expr(v)).unwrap_or_else(|| "null".to_string());
                        self.write(&format!("let {} = {};", Self::js_ident(&name.lexeme), val));
                    }
                }
                self.newline();
            }
        }
    }

    fn emit_stmt_inline(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Block { statements } => {
                self.write("{");
                self.newline();
                self.indent += 1;
                for s in statements {
                    self.emit_stmt(s);
                }
                self.indent -= 1;
                let ind = self.indent_str();
                self.write(&ind);
                self.write("}");
            }
            other => {
                self.write("{");
                self.newline();
                self.indent += 1;
                self.emit_stmt(other);
                self.indent -= 1;
                let ind = self.indent_str();
                self.write(&ind);
                self.write("}");
            }
        }
    }

    fn emit_component(&mut self, name: &str, bindings: &[Stmt], view: &[core::ast::TemplateNode]) {
        use core::ast::{BindingQualifier, Stmt as S};
        let ind = self.indent_str();
        // function <Name>_create(host) { ... }
        self.write(&format!("{}function {}_create(host) {{\n", ind, Self::js_ident(name)));
        self.indent += 1;
        let ind2 = self.indent_str();
        // emit state declarations
        for b in bindings {
            if let S::QualifiedBinding { qualifier, name: n, value, .. } = b {
                let val = value.as_deref().map(|v| self.emit_expr(v)).unwrap_or_else(|| "undefined".to_string());
                match qualifier {
                    BindingQualifier::State => {
                        self.write(&format!("{}let {} = {};\n", ind2, Self::js_ident(&n.lexeme), val));
                    }
                    BindingQualifier::Memo => {
                        self.write(&format!("{}let {} = {};\n", ind2, Self::js_ident(&n.lexeme), val));
                    }
                    BindingQualifier::Prop => {
                        self.write(&format!("{}const {} = host.getAttribute(\"{}\");\n", ind2, Self::js_ident(&n.lexeme), n.lexeme));
                    }
                    BindingQualifier::Ref => {
                        self.write(&format!("{}let {} = {};\n", ind2, Self::js_ident(&n.lexeme), val));
                    }
                }
                // generate set_X updater for state bindings
                if matches!(qualifier, BindingQualifier::State) {
                    let vname = Self::js_ident(&n.lexeme);
                    self.write(&format!(
                        "{}function set_{}(v) {{ {} = v; __flush_{}(); }}\n",
                        ind2, vname, vname, Self::js_ident(name)
                    ));
                }
            }
        }
        // emit DOM construction from view
        if !view.is_empty() {
            self.write(&format!("{}const __root = document.createDocumentFragment();\n", ind2));
            for node in view {
                self.emit_template_node_create(node, "__root");
            }
            self.write(&format!("{}host.appendChild(__root);\n", ind2));
        }
        // flush function for batched updates
        self.write(&format!("{}function __flush_{}() {{}}\n", ind2, Self::js_ident(name)));
        self.indent -= 1;
        self.write(&format!("{}}}\n", ind));
    }

    fn emit_template_node_create(&mut self, node: &core::ast::TemplateNode, parent: &str) {
        use core::ast::TemplateNode;
        let ind = self.indent_str();
        match node {
            TemplateNode::Element { tag, attrs, children } => {
                let var = format!("__el_{}", tag);
                self.write(&format!("{}const {} = document.createElement(\"{}\");\n", ind, var, tag));
                for attr in attrs {
                    use core::ast::TemplateAttrValue;
                    match &attr.value {
                        TemplateAttrValue::Static(s) => {
                            self.write(&format!("{}{}.setAttribute(\"{}\", \"{}\");\n", ind, var, attr.name, s));
                        }
                        TemplateAttrValue::Dynamic(e) => {
                            let js = self.emit_expr(e);
                            if attr.name.starts_with("on:") {
                                let event = &attr.name[3..];
                                self.write(&format!("{}{}.addEventListener(\"{}\", {});\n", ind, var, event, js));
                            } else {
                                self.write(&format!("{}{}.setAttribute(\"{}\", {});\n", ind, var, attr.name, js));
                            }
                        }
                        TemplateAttrValue::EventHandler(e) => {
                            let js = self.emit_expr(e);
                            let event = attr.name.strip_prefix("on:").unwrap_or(&attr.name);
                            self.write(&format!("{}{}.addEventListener(\"{}\", {});\n", ind, var, event, js));
                        }
                        TemplateAttrValue::Flag => {
                            self.write(&format!("{}{}.setAttribute(\"{}\", \"\");\n", ind, var, attr.name));
                        }
                    }
                }
                for child in children {
                    self.emit_template_node_create(child, &var);
                }
                self.write(&format!("{}{}.appendChild({});\n", ind, parent, var));
            }
            TemplateNode::Text(t) => {
                self.write(&format!("{}{}.appendChild(document.createTextNode(\"{}\"));\n", ind, parent, t));
            }
            TemplateNode::Expr(e) => {
                let js = self.emit_expr(e);
                self.write(&format!("{}{}.appendChild(document.createTextNode(String({})));\n", ind, parent, js));
            }
            TemplateNode::Component { name: cname, .. } => {
                self.write(&format!("{}{}_create({});\n", ind, Self::js_ident(cname), parent));
            }
            _ => {}
        }
    }

    fn emit_match(
        &mut self,
        expr: &Expr,
        cases: &[(MatchPattern, Option<Expr>, Stmt)],
    ) {
        let ind = self.indent_str();
        let subject = self.emit_expr(expr);
        let tmp = "__match_val";
        self.write(&format!("{}const {} = {};\n", ind, tmp, subject));
        let mut first = true;
        for (pattern, guard, body) in cases {
            let ind = self.indent_str();
            let cond = self.emit_match_condition(pattern, tmp);
            let preamble = self.emit_match_bindings(pattern, tmp);
            let guard_str = guard
                .as_ref()
                .map(|g| format!(" && ({})", self.emit_expr(g)))
                .unwrap_or_default();
            let kw = if first { "if" } else { " else if" };
            first = false;
            if cond == "true" && guard_str.is_empty() {
                self.write(&format!("{}{} (true) ", ind, kw));
            } else if cond == "true" {
                self.write(&format!("{}{} ({}) ", ind, kw, &guard_str[4..]));
            } else {
                self.write(&format!("{}{} ({}{})", ind, kw, cond, guard_str));
                self.write(" ");
            }
            self.write("{");
            self.newline();
            self.indent += 1;
            if !preamble.is_empty() {
                let ind2 = self.indent_str();
                self.write(&format!("{}{}\n", ind2, preamble));
            }
            self.emit_stmt(body);
            self.indent -= 1;
            let ind = self.indent_str();
            self.write(&format!("{}}}", ind));
        }
        self.newline();
    }

    fn emit_match_condition(&self, pattern: &MatchPattern, subject: &str) -> String {
        match pattern {
            MatchPattern::Wildcard => "true".to_string(),
            MatchPattern::Variable(_) | MatchPattern::Binding(_) => "true".to_string(),
            MatchPattern::Literal(val) => {
                let lit = Self::emit_value_static(val);
                format!("{} === {}", subject, lit)
            }
            MatchPattern::EnumVariant { variant, params, .. } => {
                let tag = &variant.lexeme;
                let base = format!("{}.tag === \"{}\"", subject, tag);
                if let Some(pats) = params {
                    let sub_conds: Vec<String> = pats
                        .iter()
                        .enumerate()
                        .filter_map(|(i, p)| {
                            let sub = format!("{}.payload[{}]", subject, i);
                            let c = self.emit_match_condition(p, &sub);
                            if c == "true" { None } else { Some(c) }
                        })
                        .collect();
                    if sub_conds.is_empty() {
                        base
                    } else {
                        format!("{} && {}", base, sub_conds.join(" && "))
                    }
                } else {
                    base
                }
            }
            MatchPattern::Tuple(pats) => {
                let sub_conds: Vec<String> = pats
                    .iter()
                    .enumerate()
                    .filter_map(|(i, p)| {
                        let sub = format!("{}[{}]", subject, i);
                        let c = self.emit_match_condition(p, &sub);
                        if c == "true" { None } else { Some(c) }
                    })
                    .collect();
                if sub_conds.is_empty() {
                    "true".to_string()
                } else {
                    sub_conds.join(" && ")
                }
            }
        }
    }

    fn emit_match_bindings(&self, pattern: &MatchPattern, subject: &str) -> String {
        match pattern {
            MatchPattern::Variable(tok) | MatchPattern::Binding(tok) => {
                let name = Self::js_ident(&tok.lexeme);
                format!("const {} = {};", name, subject)
            }
            MatchPattern::EnumVariant { params: Some(pats), .. } => {
                let bindings: Vec<String> = pats
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let sub = format!("{}.payload[{}]", subject, i);
                        self.emit_match_bindings(p, &sub)
                    })
                    .filter(|s| !s.is_empty())
                    .collect();
                bindings.join(" ")
            }
            MatchPattern::Tuple(pats) => {
                let bindings: Vec<String> = pats
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let sub = format!("{}[{}]", subject, i);
                        self.emit_match_bindings(p, &sub)
                    })
                    .filter(|s| !s.is_empty())
                    .collect();
                bindings.join(" ")
            }
            _ => String::new(),
        }
    }

    fn emit_iflet_condition(&self, pattern: &MatchPattern, subject: &str) -> String {
        self.emit_match_condition(pattern, subject)
    }

    fn emit_iflet_bindings(&self, pattern: &MatchPattern, subject: &str) -> String {
        self.emit_match_bindings(pattern, subject)
    }

    // ── expressions ──────────────────────────────────────────────────────────

    fn emit_expr(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Literal(val) => Self::emit_value_static(val),

            Expr::Variable { name } => Self::js_ident(&name.lexeme),

            Expr::Grouping { expression } => {
                let inner = self.emit_expr(expression);
                format!("({})", inner)
            }

            Expr::Binary { left, operator, right } => {
                let l = self.emit_expr(left);
                let r = self.emit_expr(right);
                let op = Self::map_operator(&operator.lexeme);
                format!("{} {} {}", l, op, r)
            }

            Expr::Logical { left, operator, right } => {
                let l = self.emit_expr(left);
                let r = self.emit_expr(right);
                let op = match operator.lexeme.as_str() {
                    "and" => "&&",
                    "or" => "||",
                    other => other,
                };
                format!("{} {} {}", l, op, r)
            }

            Expr::Unary { operator, right } => {
                let r = self.emit_expr(right);
                let op = match operator.lexeme.as_str() {
                    "not" => "!",
                    other => other,
                };
                format!("{}{}", op, r)
            }

            Expr::Call { callee, arguments, .. } => {
                let fn_expr = self.emit_expr(callee);
                let args: Vec<String> =
                    arguments.iter().map(|a| self.emit_expr(a)).collect();
                format!("{}({})", fn_expr, args.join(", "))
            }

            Expr::FieldAccess { object, field } => {
                let obj = self.emit_expr(object);
                let f = Self::js_ident(&field.lexeme);
                format!("{}.{}", obj, f)
            }

            Expr::Array(items) => {
                let elems: Vec<String> = items.iter().map(|e| self.emit_expr(e)).collect();
                format!("[{}]", elems.join(", "))
            }

            Expr::Tuple(items) => {
                let elems: Vec<String> = items.iter().map(|e| self.emit_expr(e)).collect();
                format!("[{}]", elems.join(", "))
            }

            Expr::StructInit { name, fields } => {
                let cname = &name.lexeme;
                let field_args: Vec<String> = fields
                    .iter()
                    .map(|(_, v)| self.emit_expr(v))
                    .collect();
                format!("new {}({})", cname, field_args.join(", "))
            }

            Expr::EnumInit { variant, values, .. } => {
                let tag = &variant.lexeme;
                if values.is_empty() {
                    format!("{{ tag: \"{}\" }}", tag)
                } else {
                    let vals: Vec<String> = values.iter().map(|v| self.emit_expr(v)).collect();
                    format!("{{ tag: \"{}\", payload: [{}] }}", tag, vals.join(", "))
                }
            }

            Expr::InterpolatedString(parts) => {
                let mut content = String::new();
                for part in parts {
                    match part {
                        InterpolatedPart::Literal(s) => {
                            content.push_str(&s.replace('`', "\\`").replace("${", "\\${"));
                        }
                        InterpolatedPart::Expr { expr, .. } => {
                            let inner = self.emit_expr(expr);
                            content.push_str(&format!("${{{}}}", inner));
                        }
                    }
                }
                format!("`{}`", content)
            }

            Expr::Cast { object, .. } => self.emit_expr(object),

            Expr::Try(inner) => {
                let val = self.emit_expr(inner);
                // Emit as: (__art_try(val)) — a runtime helper
                format!("__art_try({})", val)
            }

            Expr::Weak(inner) | Expr::Unowned(inner) => self.emit_expr(inner),

            Expr::WeakUpgrade(inner) => {
                let val = self.emit_expr(inner);
                format!("{}?.deref()", val)
            }

            Expr::UnownedAccess(inner) => self.emit_expr(inner),

            Expr::SpawnActor { body } => {
                let mut inner = CodegenJs::new(CodegenOptions {
                    emit_source_map: false,
                    ..Default::default()
                });
                for s in body {
                    inner.emit_stmt(s);
                }
                let src = inner.output.replace('`', "\\`");
                format!(
                    "new Worker(URL.createObjectURL(new Blob([`{}`], {{ type: 'application/javascript' }})))",
                    src
                )
            }

            Expr::Template(nodes) => self.emit_template_iife(nodes),
        }
    }

    // ── ArtML template codegen ────────────────────────────────────────────────

    fn emit_template_iife(&mut self, nodes: &[TemplateNode]) -> String {
        // Generate an IIFE that builds and returns a DOM node/fragment.
        let mut buf = String::new();
        let mut counter = 0usize;

        buf.push_str("(() => {\n");

        if nodes.len() == 1 {
            let var = format!("__el_{}", counter);
            counter += 1;
            Self::emit_template_node_into(&mut buf, &var, &mut counter, nodes[0].clone(), self);
            buf.push_str(&format!("  return {};\n", var));
        } else {
            let frag = format!("__frag_{}", counter);
            counter += 1;
            buf.push_str(&format!("  const {} = document.createDocumentFragment();\n", frag));
            for node in nodes {
                let child_var = format!("__el_{}", counter);
                counter += 1;
                Self::emit_template_node_into(&mut buf, &child_var, &mut counter, node.clone(), self);
                buf.push_str(&format!("  {}.appendChild({});\n", frag, child_var));
            }
            buf.push_str(&format!("  return {};\n", frag));
        }

        buf.push_str("})()");
        buf
    }

    fn emit_template_node_into(
        buf: &mut String,
        var: &str,
        counter: &mut usize,
        node: TemplateNode,
        cg: &mut CodegenJs,
    ) {
        match node {
            TemplateNode::Element { tag, attrs, children } => {
                buf.push_str(&format!("  const {} = document.createElement(\"{}\");\n", var, tag));
                Self::emit_attrs_into(buf, var, counter, &attrs, cg);
                Self::emit_children_into(buf, var, counter, &children, cg);
            }

            TemplateNode::Component { name, attrs, children } => {
                let props: Vec<String> = attrs
                    .iter()
                    .map(|a| {
                        let val = match &a.value {
                            TemplateAttrValue::Static(s) => format!("\"{}\"", CodegenJs::escape_string(s)),
                            TemplateAttrValue::Dynamic(e) | TemplateAttrValue::EventHandler(e) => cg.emit_expr(e),
                            TemplateAttrValue::Flag => "true".to_string(),
                        };
                        format!("{}: {}", a.name, val)
                    })
                    .collect();
                let has_children = !children.is_empty();
                if has_children {
                    let frag = format!("__frag_{}", counter);
                    *counter += 1;
                    buf.push_str(&format!("  const {} = document.createDocumentFragment();\n", frag));
                    Self::emit_children_into(buf, &frag, counter, &children, cg);
                    buf.push_str(&format!("  const {} = new {}({{ {}, children: {} }});\n",
                        var, name, props.join(", "), frag));
                } else {
                    buf.push_str(&format!("  const {} = new {}({{ {} }});\n", var, name, props.join(", ")));
                }
            }

            TemplateNode::Text(text) => {
                buf.push_str(&format!("  const {} = document.createTextNode(\"{}\");\n",
                    var, CodegenJs::escape_string(&text)));
            }

            TemplateNode::Expr(expr) => {
                let val = cg.emit_expr(&expr);
                buf.push_str(&format!("  const {} = document.createTextNode(String({}));\n", var, val));
            }

            TemplateNode::If { cond, then_children, else_children } => {
                let cond_js = cg.emit_expr(&cond);
                buf.push_str(&format!("  const {} = document.createDocumentFragment();\n", var));
                buf.push_str(&format!("  if ({}) {{\n", cond_js));
                let then_frag = format!("__then_{}", counter);
                *counter += 1;
                buf.push_str(&format!("    const {} = document.createDocumentFragment();\n", then_frag));
                for child in &then_children {
                    let cv = format!("__el_{}", counter);
                    *counter += 1;
                    // indent inside if
                    let mut inner_buf = String::new();
                    Self::emit_template_node_into(&mut inner_buf, &cv, counter, child.clone(), cg);
                    for line in inner_buf.lines() {
                        buf.push_str("  ");
                        buf.push_str(line);
                        buf.push('\n');
                    }
                    buf.push_str(&format!("    {}.appendChild({});\n", then_frag, cv));
                }
                buf.push_str(&format!("    {}.appendChild({});\n", var, then_frag));
                if !else_children.is_empty() {
                    buf.push_str("  } else {\n");
                    let else_frag = format!("__else_{}", counter);
                    *counter += 1;
                    buf.push_str(&format!("    const {} = document.createDocumentFragment();\n", else_frag));
                    for child in &else_children {
                        let cv = format!("__el_{}", counter);
                        *counter += 1;
                        let mut inner_buf = String::new();
                        Self::emit_template_node_into(&mut inner_buf, &cv, counter, child.clone(), cg);
                        for line in inner_buf.lines() {
                            buf.push_str("  ");
                            buf.push_str(line);
                            buf.push('\n');
                        }
                        buf.push_str(&format!("    {}.appendChild({});\n", else_frag, cv));
                    }
                    buf.push_str(&format!("    {}.appendChild({});\n", var, else_frag));
                }
                buf.push_str("  }\n");
            }

            TemplateNode::For { var: loop_var, items, key: _, children } => {
                let items_js = cg.emit_expr(&items);
                buf.push_str(&format!("  const {} = document.createDocumentFragment();\n", var));
                buf.push_str(&format!("  for (const {} of {}) {{\n", loop_var, items_js));
                for child in &children {
                    let cv = format!("__el_{}", counter);
                    *counter += 1;
                    let mut inner_buf = String::new();
                    Self::emit_template_node_into(&mut inner_buf, &cv, counter, child.clone(), cg);
                    for line in inner_buf.lines() {
                        buf.push_str("  ");
                        buf.push_str(line);
                        buf.push('\n');
                    }
                    buf.push_str(&format!("    {}.appendChild({});\n", var, cv));
                }
                buf.push_str("  }\n");
            }

            TemplateNode::Slot { name, children } => {
                let slot_id = name.as_deref().unwrap_or("default");
                buf.push_str(&format!(
                    "  const {} = document.createElement(\"slot\");\n", var));
                if slot_id != "default" {
                    buf.push_str(&format!("  {}.setAttribute(\"name\", \"{}\");\n", var, slot_id));
                }
                Self::emit_children_into(buf, var, counter, &children, cg);
            }
        }
    }

    fn emit_attrs_into(
        buf: &mut String,
        var: &str,
        counter: &mut usize,
        attrs: &[core::ast::TemplateAttr],
        cg: &mut CodegenJs,
    ) {
        for attr in attrs {
            match &attr.value {
                TemplateAttrValue::Static(s) => {
                    buf.push_str(&format!("  {}.setAttribute(\"{}\", \"{}\");\n",
                        var, attr.name, CodegenJs::escape_string(s)));
                }
                TemplateAttrValue::Dynamic(expr) => {
                    let val = cg.emit_expr(expr);
                    buf.push_str(&format!("  {}.setAttribute(\"{}\", String({}));\n",
                        var, attr.name, val));
                }
                TemplateAttrValue::EventHandler(expr) => {
                    let event_name = attr.name.strip_prefix("on:").unwrap_or(&attr.name);
                    let handler = cg.emit_expr(expr);
                    buf.push_str(&format!("  {}.addEventListener(\"{}\", () => {{ {}; }});\n",
                        var, event_name, handler));
                }
                TemplateAttrValue::Flag => {
                    buf.push_str(&format!("  {}.setAttribute(\"{}\", \"\");\n", var, attr.name));
                }
            }
            let _ = counter; // suppress unused warning
        }
    }

    fn emit_children_into(
        buf: &mut String,
        parent: &str,
        counter: &mut usize,
        children: &[TemplateNode],
        cg: &mut CodegenJs,
    ) {
        for child in children {
            let cv = format!("__el_{}", counter);
            *counter += 1;
            Self::emit_template_node_into(buf, &cv, counter, child.clone(), cg);
            buf.push_str(&format!("  {}.appendChild({});\n", parent, cv));
        }
    }

    fn emit_value_static(val: &ArtValue) -> String {
        match val {
            ArtValue::Int(n) => n.to_string(),
            ArtValue::Float(f) => {
                if f.fract() == 0.0 {
                    format!("{:.1}", f)
                } else {
                    f.to_string()
                }
            }
            ArtValue::String(s) => format!("\"{}\"", Self::escape_string(s)),
            ArtValue::Bool(b) => b.to_string(),
            ArtValue::Optional(opt) => match opt.as_ref() {
                None => "null".to_string(),
                Some(v) => Self::emit_value_static(v),
            },
            ArtValue::Array(items) => {
                let elems: Vec<String> = items.iter().map(Self::emit_value_static).collect();
                format!("[{}]", elems.join(", "))
            }
            ArtValue::Tuple(items) => {
                let elems: Vec<String> = items.iter().map(Self::emit_value_static).collect();
                format!("[{}]", elems.join(", "))
            }
            ArtValue::EnumInstance { variant, values, .. } => {
                if values.is_empty() {
                    format!("{{ tag: \"{}\" }}", variant)
                } else {
                    let vals: Vec<String> = values.iter().map(Self::emit_value_static).collect();
                    format!("{{ tag: \"{}\", payload: [{}] }}", variant, vals.join(", "))
                }
            }
            _ => "null".to_string(),
        }
    }

    fn map_operator(op: &str) -> &str {
        match op {
            "==" => "===",
            "!=" => "!==",
            "and" => "&&",
            "or" => "||",
            "not" => "!",
            other => other,
        }
    }
}

impl Default for CodegenJs {
    fn default() -> Self {
        Self::new(CodegenOptions::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::ast::{FunctionParam, MatchPattern};
    use core::Token;
    use std::rc::Rc;

    fn tok(s: &str) -> Token {
        Token::dummy(s)
    }

    fn emit(stmts: Vec<Stmt>) -> String {
        CodegenJs::new(CodegenOptions::default()).emit_program(&stmts).code
    }

    #[test]
    fn let_int() {
        let stmts = vec![Stmt::Let {
            pattern: MatchPattern::Variable(tok("x")),
            ty: None,
            initializer: Expr::Literal(ArtValue::Int(42)),
        }];
        let js = emit(stmts);
        assert!(js.contains("const x = 42;"), "got: {}", js);
    }

    #[test]
    fn let_string() {
        let stmts = vec![Stmt::Let {
            pattern: MatchPattern::Variable(tok("greeting")),
            ty: None,
            initializer: Expr::Literal(ArtValue::String(std::sync::Arc::from("hello"))),
        }];
        let js = emit(stmts);
        assert!(js.contains("const greeting = \"hello\";"), "got: {}", js);
    }

    #[test]
    fn function_decl() {
        let body = Stmt::Block {
            statements: vec![Stmt::Return {
                value: Some(Expr::Literal(ArtValue::Int(1))),
            }],
        };
        let stmts = vec![Stmt::Function {
            name: tok("greet"),
            type_params: None,
            params: vec![FunctionParam { name: tok("name"), ty: None }],
            return_type: None,
            body: Rc::new(body),
            method_owner: None,
            is_async: false,
        }];
        let js = emit(stmts);
        assert!(js.contains("function greet(name)"), "got: {}", js);
        assert!(js.contains("return 1;"), "got: {}", js);
    }

    #[test]
    fn if_else() {
        let stmts = vec![Stmt::If {
            condition: Expr::Literal(ArtValue::Bool(true)),
            then_branch: Box::new(Stmt::Block { statements: vec![] }),
            else_branch: None,
        }];
        let js = emit(stmts);
        assert!(js.contains("if (true)"), "got: {}", js);
    }

    #[test]
    fn for_loop() {
        let stmts = vec![Stmt::For {
            element: tok("item"),
            iterator: Expr::Variable { name: tok("items") },
            body: Box::new(Stmt::Block { statements: vec![] }),
        }];
        let js = emit(stmts);
        assert!(js.contains("for (const item of items)"), "got: {}", js);
    }

    #[test]
    fn struct_decl() {
        let stmts = vec![Stmt::StructDecl {
            name: tok("Point"),
            fields: vec![(tok("x"), "Int".to_string()), (tok("y"), "Int".to_string())],
        }];
        let js = emit(stmts);
        assert!(js.contains("class Point"), "got: {}", js);
        assert!(js.contains("constructor(x, y)"), "got: {}", js);
        assert!(js.contains("this.x = x;"), "got: {}", js);
    }

    #[test]
    fn enum_decl() {
        let stmts = vec![Stmt::EnumDecl {
            name: tok("Status"),
            variants: vec![
                (tok("Ok"), Some(vec!["Int".to_string()])),
                (tok("Err"), None),
            ],
        }];
        let js = emit(stmts);
        assert!(js.contains("const Status"), "got: {}", js);
        assert!(js.contains("tag: \"Ok\""), "got: {}", js);
    }

    #[test]
    fn interpolated_string() {
        let stmts = vec![Stmt::Expression(Expr::InterpolatedString(vec![
            InterpolatedPart::Literal("Hello, ".to_string()),
            InterpolatedPart::Expr {
                expr: Box::new(Expr::Variable { name: tok("name") }),
                format: None,
            },
        ]))];
        let js = emit(stmts);
        assert!(js.contains("`Hello, ${name}`"), "got: {}", js);
    }

    #[test]
    fn array_literal() {
        let stmts = vec![Stmt::Let {
            pattern: MatchPattern::Variable(tok("arr")),
            ty: None,
            initializer: Expr::Array(vec![
                Expr::Literal(ArtValue::Int(1)),
                Expr::Literal(ArtValue::Int(2)),
            ]),
        }];
        let js = emit(stmts);
        assert!(js.contains("const arr = [1, 2];"), "got: {}", js);
    }

    #[test]
    fn binary_eq_maps_to_triple_eq() {
        assert_eq!(CodegenJs::map_operator("=="), "===");
        assert_eq!(CodegenJs::map_operator("!="), "!==");
    }

    #[test]
    fn sourcemap_emitted_when_enabled() {
        let stmts = vec![Stmt::Let {
            pattern: MatchPattern::Variable(tok("x")),
            ty: None,
            initializer: Expr::Literal(ArtValue::Int(1)),
        }];
        let out = CodegenJs::new(CodegenOptions {
            emit_source_map: true,
            source_file: Some("test.art".to_string()),
            ..Default::default()
        })
        .emit_program(&stmts);
        assert!(out.source_map.is_some());
        let sm = out.source_map.unwrap();
        assert!(sm.contains("\"version\":3"));
        assert!(sm.contains("test.art"));
    }
}
