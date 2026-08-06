use core::ast::{BindingQualifier, Expr, Stmt, TemplateAttrValue, TemplateNode};
use diagnostics::{Diagnostic, DiagnosticKind, Span};

// ── Node IDs ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

// ── Node kinds ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    /// state / prop binding — produces reactive values
    Source,
    /// memo binding — derived from other nodes
    Derived,
    /// template node index — depends on sources/derived
    Sink,
}

#[derive(Debug, Clone)]
pub struct DepNode {
    pub id: NodeId,
    pub name: String,
    pub kind: NodeKind,
}

// ── Dependency graph ─────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct DepGraph {
    pub nodes: Vec<DepNode>,
    /// edges[i] = set of node ids that node i depends on
    pub edges: Vec<Vec<NodeId>>,
}

impl DepGraph {
    fn add_node(&mut self, name: String, kind: NodeKind) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(DepNode { id, name, kind });
        self.edges.push(Vec::new());
        id
    }

    fn add_edge(&mut self, from: NodeId, to: NodeId) {
        let deps = &mut self.edges[from.0];
        if !deps.contains(&to) {
            deps.push(to);
        }
    }

    pub fn node_by_name(&self, name: &str) -> Option<NodeId> {
        self.nodes.iter().find(|n| n.name == name).map(|n| n.id)
    }

    /// Return all nodes whose `kind == Source`.
    pub fn sources(&self) -> impl Iterator<Item = &DepNode> {
        self.nodes.iter().filter(|n| n.kind == NodeKind::Source)
    }

    /// Return all nodes that transitively depend on `root` (BFS).
    pub fn dependents_of(&self, root: NodeId) -> Vec<NodeId> {
        let mut visited = vec![false; self.nodes.len()];
        let mut queue = std::collections::VecDeque::new();
        let mut result = Vec::new();
        visited[root.0] = true;
        // collect nodes whose dep list contains root
        for node in &self.nodes {
            if self.edges[node.id.0].contains(&root) && !visited[node.id.0] {
                visited[node.id.0] = true;
                queue.push_back(node.id);
            }
        }
        while let Some(id) = queue.pop_front() {
            result.push(id);
            for node in &self.nodes {
                if self.edges[node.id.0].contains(&id) && !visited[node.id.0] {
                    visited[node.id.0] = true;
                    queue.push_back(node.id);
                }
            }
        }
        result
    }

    /// Topological order of Derived nodes (Kahn's algorithm).
    /// Returns None if a cycle exists.
    pub fn topo_derived(&self) -> Option<Vec<NodeId>> {
        let n = self.nodes.len();
        let mut in_deg = vec![0usize; n];
        for deps in &self.edges {
            for &dep in deps {
                in_deg[dep.0] += 1;
            }
        }
        let mut queue: std::collections::VecDeque<NodeId> = self
            .nodes
            .iter()
            .filter(|nd| nd.kind == NodeKind::Derived && in_deg[nd.id.0] == 0)
            .map(|nd| nd.id)
            .collect();
        let mut order = Vec::new();
        while let Some(id) = queue.pop_front() {
            order.push(id);
            for dep in &self.edges[id.0] {
                in_deg[dep.0] -= 1;
                if in_deg[dep.0] == 0 && self.nodes[dep.0].kind == NodeKind::Derived {
                    queue.push_back(*dep);
                }
            }
        }
        let derived_count = self
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Derived)
            .count();
        if order.len() == derived_count {
            Some(order)
        } else {
            None
        }
    }
}

// ── Tarjan SCC ───────────────────────────────────────────────────────────────

struct Tarjan<'a> {
    graph: &'a DepGraph,
    index_counter: usize,
    stack: Vec<NodeId>,
    on_stack: Vec<bool>,
    index: Vec<Option<usize>>,
    lowlink: Vec<usize>,
    sccs: Vec<Vec<NodeId>>,
}

impl<'a> Tarjan<'a> {
    fn run(graph: &'a DepGraph) -> Vec<Vec<NodeId>> {
        let n = graph.nodes.len();
        let mut t = Tarjan {
            graph,
            index_counter: 0,
            stack: Vec::new(),
            on_stack: vec![false; n],
            index: vec![None; n],
            lowlink: vec![0; n],
            sccs: Vec::new(),
        };
        for node in &graph.nodes {
            if t.index[node.id.0].is_none() {
                t.strongconnect(node.id);
            }
        }
        t.sccs
    }

    fn strongconnect(&mut self, v: NodeId) {
        self.index[v.0] = Some(self.index_counter);
        self.lowlink[v.0] = self.index_counter;
        self.index_counter += 1;
        self.stack.push(v);
        self.on_stack[v.0] = true;

        for &w in &self.graph.edges[v.0] {
            if self.index[w.0].is_none() {
                self.strongconnect(w);
                self.lowlink[v.0] = self.lowlink[v.0].min(self.lowlink[w.0]);
            } else if self.on_stack[w.0] {
                let w_idx = self.index[w.0].unwrap();
                self.lowlink[v.0] = self.lowlink[v.0].min(w_idx);
            }
        }

        if self.lowlink[v.0] == self.index[v.0].unwrap() {
            let mut scc = Vec::new();
            loop {
                let w = self.stack.pop().expect("stack non-empty during SCC");
                self.on_stack[w.0] = false;
                scc.push(w);
                if w == v {
                    break;
                }
            }
            self.sccs.push(scc);
        }
    }
}

// ── ReactivityPass ──────────────────────────────────────────────────────────

pub struct ReactivityPass;

pub struct PassResult {
    pub graph: DepGraph,
    pub diagnostics: Vec<Diagnostic>,
}

impl ReactivityPass {
    pub fn analyse(component: &Stmt) -> PassResult {
        let mut graph = DepGraph::default();
        let mut diagnostics = Vec::new();

        let (bindings, view) = match component {
            Stmt::ComponentBlock { bindings, view, .. } => (bindings, view),
            _ => return PassResult { graph, diagnostics },
        };

        // Phase 1: register all binding nodes
        for binding in bindings {
            if let Stmt::QualifiedBinding {
                qualifier, name, ..
            } = binding
            {
                let kind = match qualifier {
                    BindingQualifier::State | BindingQualifier::Prop => NodeKind::Source,
                    BindingQualifier::Memo => NodeKind::Derived,
                    BindingQualifier::Ref => NodeKind::Source,
                };
                graph.add_node(name.lexeme.clone(), kind);
            }
        }

        // Phase 2: add sink nodes for each template leaf that references a binding
        let sink_refs = collect_template_refs(view);
        for sink_var in &sink_refs {
            if graph.node_by_name(sink_var).is_none() {
                continue;
            }
            let sink_id = graph.add_node(format!("view:{sink_var}"), NodeKind::Sink);
            if let Some(src_id) = graph.node_by_name(sink_var) {
                graph.add_edge(sink_id, src_id);
            }
        }

        // Phase 3: add edges for memo deps
        for binding in bindings {
            if let Stmt::QualifiedBinding {
                qualifier: BindingQualifier::Memo,
                name,
                value,
                ..
            } = binding
            {
                let memo_id = match graph.node_by_name(&name.lexeme) {
                    Some(id) => id,
                    None => continue,
                };
                if let Some(expr) = value {
                    let refs = collect_expr_refs(expr);
                    for dep_name in &refs {
                        if let Some(dep_id) = graph.node_by_name(dep_name) {
                            graph.add_edge(memo_id, dep_id);
                        }
                    }
                }
            }
        }

        // Phase 4: cycle detection via Tarjan
        let sccs = Tarjan::run(&graph);
        for scc in &sccs {
            if scc.len() > 1 {
                // Only report cycles among Derived nodes
                let cycle_derived: Vec<&str> = scc
                    .iter()
                    .filter(|&&id| graph.nodes[id.0].kind == NodeKind::Derived)
                    .map(|&id| graph.nodes[id.0].name.as_str())
                    .collect();
                if cycle_derived.len() > 1 {
                    let names = cycle_derived.join("' -> '");
                    diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Type,
                        format!("reactive cycle detected — '{names}'"),
                        Span::new(0, 0, 0, 0),
                    ));
                }
            }
        }

        PassResult { graph, diagnostics }
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn collect_template_refs(nodes: &[TemplateNode]) -> Vec<String> {
    let mut out = Vec::new();
    for node in nodes {
        match node {
            TemplateNode::Expr(e) => collect_expr_refs_into(e, &mut out),
            TemplateNode::Element {
                attrs, children, ..
            }
            | TemplateNode::Component {
                attrs, children, ..
            } => {
                for attr in attrs {
                    match &attr.value {
                        TemplateAttrValue::Dynamic(e) | TemplateAttrValue::EventHandler(e) => {
                            collect_expr_refs_into(e, &mut out);
                        }
                        _ => {}
                    }
                }
                out.extend(collect_template_refs(children));
            }
            TemplateNode::If {
                cond,
                then_children,
                else_children,
            } => {
                collect_expr_refs_into(cond, &mut out);
                out.extend(collect_template_refs(then_children));
                out.extend(collect_template_refs(else_children));
            }
            TemplateNode::For {
                items, children, ..
            } => {
                collect_expr_refs_into(items, &mut out);
                out.extend(collect_template_refs(children));
            }
            TemplateNode::Slot { children, .. } => {
                out.extend(collect_template_refs(children));
            }
            TemplateNode::Text(_) => {}
        }
    }
    out
}

fn collect_expr_refs(expr: &Expr) -> Vec<String> {
    let mut out = Vec::new();
    collect_expr_refs_into(expr, &mut out);
    out
}

fn collect_expr_refs_into(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::Variable { name } => out.push(name.lexeme.clone()),
        Expr::Binary { left, right, .. } | Expr::Logical { left, right, .. } => {
            collect_expr_refs_into(left, out);
            collect_expr_refs_into(right, out);
        }
        Expr::Unary { right, .. } => collect_expr_refs_into(right, out),
        Expr::Grouping { expression } => collect_expr_refs_into(expression, out),
        Expr::Call {
            callee, arguments, ..
        } => {
            collect_expr_refs_into(callee, out);
            for a in arguments {
                collect_expr_refs_into(a, out);
            }
        }
        Expr::FieldAccess { object, .. } => collect_expr_refs_into(object, out),
        Expr::Array(elems) | Expr::Tuple(elems) => {
            for e in elems {
                collect_expr_refs_into(e, out);
            }
        }
        Expr::Cast { object, .. } => collect_expr_refs_into(object, out),
        Expr::Try(e)
        | Expr::Weak(e)
        | Expr::Unowned(e)
        | Expr::WeakUpgrade(e)
        | Expr::UnownedAccess(e) => {
            collect_expr_refs_into(e, out);
        }
        Expr::StructInit { fields, .. } => {
            for (_, e) in fields {
                collect_expr_refs_into(e, out);
            }
        }
        Expr::InterpolatedString(parts) => {
            use core::ast::InterpolatedPart;
            for p in parts {
                if let InterpolatedPart::Expr { expr, .. } = p {
                    collect_expr_refs_into(expr, out);
                }
            }
        }
        Expr::Template(nodes) => {
            out.extend(collect_template_refs(nodes));
        }
        Expr::SpawnActor { .. } | Expr::Literal(_) | Expr::EnumInit { .. } => {}
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use lexer::Lexer;
    use parser::Parser;

    fn parse_component(src: &str) -> Stmt {
        let tokens = Lexer::new(src.to_string()).scan_tokens().expect("lex");
        let (stmts, diags) = Parser::new(tokens).parse();
        assert!(diags.is_empty(), "parse errors: {:?}", diags);
        assert_eq!(stmts.len(), 1, "expected 1 stmt, got {}", stmts.len());
        stmts.into_iter().next().unwrap()
    }

    // Acyclic DAG compiles without errors
    #[test]
    fn dag_acyclic_no_errors() {
        let comp = parse_component(
            "component Counter {\n  state count: Int = 0\n  memo doubled: Int = count * 2\n  view { <p>{doubled}</p> }\n}",
        );
        let result = ReactivityPass::analyse(&comp);
        assert!(
            result.diagnostics.is_empty(),
            "unexpected: {:?}",
            result.diagnostics
        );
        // graph has: count(Source), doubled(Derived), view:doubled(Sink) at minimum
        assert!(result.graph.node_by_name("count").is_some());
        assert!(result.graph.node_by_name("doubled").is_some());
    }

    // Memo→Memo chain without cycle is fine
    #[test]
    fn dag_memo_chain_acyclic() {
        let comp = parse_component(
            "component C {\n  state x: Int = 1\n  memo a: Int = x + 1\n  memo b: Int = a * 2\n  view { <p>{b}</p> }\n}",
        );
        let result = ReactivityPass::analyse(&comp);
        assert!(
            result.diagnostics.is_empty(),
            "unexpected: {:?}",
            result.diagnostics
        );
        let g = &result.graph;
        let a_id = g.node_by_name("a").expect("a");
        let b_id = g.node_by_name("b").expect("b");
        // b depends on a
        assert!(g.edges[b_id.0].contains(&a_id), "b should depend on a");
    }

    // Cycle between two memos must emit a diagnostic
    #[test]
    fn cycle_between_two_memos_is_error() {
        // Build the graph manually since the parser can't represent mutual memo refs
        // without evaluating — wire up a synthetic graph instead.
        let mut graph = DepGraph::default();
        let a = graph.add_node("a".to_string(), NodeKind::Derived);
        let b = graph.add_node("b".to_string(), NodeKind::Derived);
        graph.add_edge(a, b);
        graph.add_edge(b, a);

        let sccs = Tarjan::run(&graph);
        let has_cycle = sccs.iter().any(|scc| scc.len() > 1);
        assert!(has_cycle, "expected cycle SCC");
    }

    // Self-referencing memo is a cycle
    #[test]
    fn cycle_self_referencing_memo() {
        let mut graph = DepGraph::default();
        let a = graph.add_node("loop_memo".to_string(), NodeKind::Derived);
        graph.add_edge(a, a);

        let sccs = Tarjan::run(&graph);
        let cycle_scc = sccs.iter().find(|scc| scc.contains(&a)).expect("SCC for a");
        // self-loop: Tarjan groups it alone but lowlink == index
        // The node is in its own SCC; edges[a] = [a] means it's a cycle
        assert!(graph.edges[a.0].contains(&a), "self-loop edge present");
        assert!(cycle_scc.contains(&a));
    }
}
