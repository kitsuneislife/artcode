use std::collections::{HashMap, HashSet};

use core::ast::ArtValue;

use super::Interpreter;

#[derive(Debug, Clone, PartialEq)]
pub struct CycleReport {
    pub weak_total: usize,
    pub weak_alive: usize,
    pub weak_dead: usize,
    pub unowned_total: usize,
    pub unowned_dangling: usize,
    pub objects_finalized: usize,
    pub heap_alive: usize,
    pub avg_out_degree: f32,
    pub avg_in_degree: f32,
    pub candidate_owner_edges: Vec<(u64, u64)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CycleDetectionResult {
    pub cycles: Vec<CycleInfo>,
    pub weak_dead: Vec<u64>,
    pub unowned_dangling: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CycleInfo {
    pub nodes: Vec<u64>,
    pub isolated: bool,
    pub suggested_break_edges: Vec<(u64, u64)>,
    pub reachable_from_root: bool,
    pub leak_candidate: bool,
    pub ranked_suggestions: Vec<(u64, u64, u32)>,
}

impl Interpreter {
    // Protótipo: coleta ids de weak/unowned mortos; sem grafo real ainda.
    pub fn detect_cycles(&mut self) -> CycleDetectionResult {
        let mut weak_dead: Vec<u64> = Vec::new();
        let mut unowned_dangling: Vec<u64> = Vec::new();
        fn scan_ids(
            v: &ArtValue,
            this: &Interpreter,
            weak_dead: &mut Vec<u64>,
            unowned_dangling: &mut Vec<u64>,
        ) {
            match v {
                ArtValue::WeakRef(h) if !this.is_object_alive(h.0) => {
                    weak_dead.push(h.0);
                }
                ArtValue::UnownedRef(h) if !this.is_object_alive(h.0) => {
                    unowned_dangling.push(h.0);
                }
                ArtValue::HeapComposite(h) => {
                    if let Some(obj) = this.heap_objects.get(&h.0) {
                        scan_ids(&obj.value, this, weak_dead, unowned_dangling);
                    }
                }
                ArtValue::Array(a) => {
                    for e in a {
                        scan_ids(e, this, weak_dead, unowned_dangling)
                    }
                }
                ArtValue::StructInstance { fields, .. } => {
                    for val in fields.values() {
                        scan_ids(val, this, weak_dead, unowned_dangling)
                    }
                }
                ArtValue::EnumInstance { values, .. } => {
                    for val in values {
                        scan_ids(val, this, weak_dead, unowned_dangling)
                    }
                }
                _ => {}
            }
        }
        for v in self.environment.borrow().values.values() {
            scan_ids(v, self, &mut weak_dead, &mut unowned_dangling);
        }
        // Build edge graph using heap ids (alive objects only)
        let mut edges: HashMap<u64, Vec<u64>> = HashMap::new();
        let mut incoming: HashMap<u64, Vec<u64>> = HashMap::new();
        for (id, obj) in self.heap_objects.iter() {
            if !obj.alive {
                continue;
            }
            match &obj.value {
                ArtValue::Array(a) => {
                    for child in a {
                        if let ArtValue::HeapComposite(h) = child
                            && let Some(c) = self.heap_objects.get(&h.0)
                            && c.alive
                        {
                            edges.entry(*id).or_default().push(h.0);
                            incoming.entry(h.0).or_default().push(*id);
                        }
                    }
                }
                ArtValue::StructInstance { fields, .. } => {
                    for child in fields.values() {
                        if let ArtValue::HeapComposite(h) = child
                            && let Some(c) = self.heap_objects.get(&h.0)
                            && c.alive
                        {
                            edges.entry(*id).or_default().push(h.0);
                            incoming.entry(h.0).or_default().push(*id);
                        }
                    }
                }
                ArtValue::EnumInstance { values, .. } => {
                    for child in values {
                        if let ArtValue::HeapComposite(h) = child
                            && let Some(c) = self.heap_objects.get(&h.0)
                            && c.alive
                        {
                            edges.entry(*id).or_default().push(h.0);
                            incoming.entry(h.0).or_default().push(*id);
                        }
                    }
                }
                _ => {}
            }
        }
        // Roots: alive objects not pointed to by any other object
        let mut all_ids: HashSet<u64> = self
            .heap_objects
            .iter()
            .filter(|(_, o)| o.alive)
            .map(|(id, _)| *id)
            .collect();
        for tgt in incoming.keys() {
            all_ids.remove(tgt);
        }
        let roots: Vec<u64> = all_ids.into_iter().collect();
        // Tarjan SCC over alive ids
        let mut id_vec: Vec<u64> = self
            .heap_objects
            .iter()
            .filter(|(_, o)| o.alive)
            .map(|(id, _)| *id)
            .collect();
        id_vec.sort_unstable();
        let mut pos: HashMap<u64, usize> = HashMap::new();
        for (i, id) in id_vec.iter().enumerate() {
            pos.insert(*id, i);
        }
        let n = id_vec.len();
        let mut index = 0usize;
        let mut indices = vec![usize::MAX; n];
        let mut lowlink = vec![0usize; n];
        let mut on_stack = vec![false; n];
        let mut stack: Vec<usize> = Vec::new();
        let mut sccs: Vec<Vec<usize>> = Vec::new();
        #[allow(clippy::too_many_arguments)]
        fn strongconnect(
            u: usize,
            index: &mut usize,
            indices: &mut [usize],
            low: &mut [usize],
            stack: &mut Vec<usize>,
            on: &mut [bool],
            edges: &HashMap<u64, Vec<u64>>,
            id_vec: &[u64],
            pos: &HashMap<u64, usize>,
            sccs: &mut Vec<Vec<usize>>,
        ) {
            indices[u] = *index;
            low[u] = *index;
            *index += 1;
            stack.push(u);
            on[u] = true;
            if let Some(neigh_ids) = edges.get(&id_vec[u]) {
                for vid in neigh_ids {
                    if let Some(&v) = pos.get(vid) {
                        if indices[v] == usize::MAX {
                            strongconnect(
                                v, index, indices, low, stack, on, edges, id_vec, pos, sccs,
                            );
                            low[u] = low[u].min(low[v]);
                        } else if on[v] {
                            low[u] = low[u].min(indices[v]);
                        }
                    }
                }
            }
            if low[u] == indices[u] {
                let mut comp = Vec::new();
                while let Some(w) = stack.pop() {
                    on[w] = false;
                    comp.push(w);
                    if w == u {
                        break;
                    }
                }
                if comp.len() > 1 {
                    sccs.push(comp);
                }
            }
        }
        for u in 0..n {
            if indices[u] == usize::MAX {
                strongconnect(
                    u,
                    &mut index,
                    &mut indices,
                    &mut lowlink,
                    &mut stack,
                    &mut on_stack,
                    &edges,
                    &id_vec,
                    &pos,
                    &mut sccs,
                );
            }
        }
        // Reachability from roots
        let mut reachable = vec![false; n];
        fn dfs(
            u: usize,
            edges: &HashMap<u64, Vec<u64>>,
            id_vec: &[u64],
            pos: &HashMap<u64, usize>,
            seen: &mut [bool],
        ) {
            if seen[u] {
                return;
            }
            seen[u] = true;
            if let Some(neigh) = edges.get(&id_vec[u]) {
                for vid in neigh {
                    if let Some(&v) = pos.get(vid) {
                        dfs(v, edges, id_vec, pos, seen);
                    }
                }
            }
        }
        for r in &roots {
            if let Some(&u) = pos.get(r) {
                dfs(u, &edges, &id_vec, &pos, &mut reachable);
            }
        }
        // Classify cycles
        let mut cycles_info = Vec::new();
        let mut leaks = 0usize;
        for comp in sccs {
            let set: HashSet<usize> = comp.iter().cloned().collect();
            let mut isolated = true;
            for &node in &comp {
                if let Some(ins) = incoming.get(&id_vec[node])
                    && ins
                        .iter()
                        .any(|p| pos.get(p).map(|&pi| !set.contains(&pi)).unwrap_or(true))
                {
                    isolated = false;
                    break;
                }
            }
            let reachable_from_root = comp.iter().any(|n| reachable[*n]);
            let leak_candidate = isolated && !reachable_from_root;
            if leak_candidate {
                leaks += 1;
            }
            let suggestions = comp
                .first()
                .and_then(|first| {
                    edges.get(&id_vec[*first]).map(|outs| {
                        outs.iter()
                            .filter_map(|cid| {
                                if let Some(&ci) = pos.get(cid) {
                                    if set.contains(&ci) {
                                        Some((id_vec[*first], *cid))
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .unwrap_or_default();
            let mut in_counts: HashMap<usize, u32> = HashMap::new();
            for &nidx in &comp {
                if let Some(ins) = incoming.get(&id_vec[nidx]) {
                    for pid in ins {
                        if let Some(&pi) = pos.get(pid)
                            && set.contains(&pi)
                        {
                            *in_counts.entry(nidx).or_insert(0) += 1;
                        }
                    }
                }
            }
            let mut ranked = Vec::new();
            for &nidx in &comp {
                if let Some(outs) = edges.get(&id_vec[nidx]) {
                    let internal: Vec<u64> = outs
                        .iter()
                        .copied()
                        .filter(|cid| pos.get(cid).map(|ci| set.contains(ci)).unwrap_or(false))
                        .collect();
                    let out_deg = internal.len() as u32;
                    for tgt in internal {
                        if let Some(&ti) = pos.get(&tgt) {
                            let score = out_deg + *in_counts.get(&ti).unwrap_or(&0);
                            ranked.push((id_vec[nidx], tgt, score));
                        }
                    }
                }
            }
            ranked.sort_by_key(|x| std::cmp::Reverse(x.2));
            ranked.truncate(3);
            cycles_info.push(CycleInfo {
                nodes: comp.iter().map(|n| id_vec[*n]).collect(),
                isolated,
                suggested_break_edges: suggestions,
                reachable_from_root,
                leak_candidate,
                ranked_suggestions: ranked,
            });
        }
        self.cycle_leaks_detected += leaks;
        CycleDetectionResult {
            cycles: cycles_info,
            weak_dead,
            unowned_dangling,
        }
    }

    pub fn detect_cycles_json(&mut self) -> String {
        let summary = self.cycle_report();
        let result = self.detect_cycles();
        let mut s = String::from("{");
        use std::fmt::Write;
        let owner_edges = summary
            .candidate_owner_edges
            .iter()
            .map(|(a, b)| format!("[{},{}]", a, b))
            .collect::<Vec<_>>()
            .join(",");
        let _ = write!(
            s,
            "\"summary\":{{\"weak_total\":{},\"weak_alive\":{},\"weak_dead\":{},\"unowned_total\":{},\"unowned_dangling\":{},\"objects_finalized\":{},\"heap_alive\":{},\"avg_out_degree\":{:.2},\"avg_in_degree\":{:.2},\"candidate_owner_edges\":[{}],\"cycle_leaks_detected\":{}}}",
            summary.weak_total,
            summary.weak_alive,
            summary.weak_dead,
            summary.unowned_total,
            summary.unowned_dangling,
            summary.objects_finalized,
            summary.heap_alive,
            summary.avg_out_degree,
            summary.avg_in_degree,
            owner_edges,
            self.cycle_leaks_detected
        );
        s.push(',');
        let _ = write!(
            s,
            "\"weak_dead_ids\":[{}]",
            result
                .weak_dead
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        s.push(',');
        let _ = write!(
            s,
            "\"unowned_dangling_ids\":[{}]",
            result
                .unowned_dangling
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        s.push(',');
        s.push_str("\"cycles\":[");
        for (i, c) in result.cycles.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let nodes = c
                .nodes
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let sugg = c
                .suggested_break_edges
                .iter()
                .map(|(a, b)| format!("[{},{}]", a, b))
                .collect::<Vec<_>>()
                .join(",");
            let ranked = c
                .ranked_suggestions
                .iter()
                .map(|(a, b, sc)| format!("[{},{} ,{}]", a, b, sc))
                .collect::<Vec<_>>()
                .join(",");
            let _ = write!(
                s,
                "{{\"nodes\":[{}],\"isolated\":{},\"reachable_from_root\":{},\"leak_candidate\":{},\"suggested_break_edges\":[{}],\"ranked_suggestions\":[{}]}}",
                nodes, c.isolated, c.reachable_from_root, c.leak_candidate, sugg, ranked
            );
        }
        s.push_str("]}");
        s
    }

    pub fn detect_cycles_json_pretty(&mut self) -> String {
        let mut raw = self.detect_cycles_json();
        let mut out = String::new();
        let mut indent = 0usize;
        let bytes: Vec<char> = raw.drain(..).collect();
        let mut i = 0;
        let len = bytes.len();
        while i < len {
            let c = bytes[i];
            match c {
                '{' | '[' => {
                    out.push(c);
                    indent += 1;
                    out.push('\n');
                    out.push_str(&"  ".repeat(indent));
                }
                '}' | ']' => {
                    indent = indent.saturating_sub(1);
                    out.push('\n');
                    out.push_str(&"  ".repeat(indent));
                    out.push(c);
                }
                ',' => {
                    out.push(c);
                    out.push('\n');
                    out.push_str(&"  ".repeat(indent));
                }
                ':' => {
                    out.push(':');
                    out.push(' ');
                }
                _ => out.push(c),
            }
            i += 1;
        }
        out
    }
}
