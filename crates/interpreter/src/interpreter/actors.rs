use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

use core::ast::{ArtValue, Stmt};
use core::environment::Environment;

use super::Interpreter;

#[derive(Clone)]
pub struct ActorState {
    pub id: u32,
    pub mailbox: Mailbox,
    pub body: VecDeque<Stmt>,
    pub env: Rc<RefCell<Environment>>,
    pub finished: bool,
    pub parked: bool,
    pub mailbox_limit: usize,
}

/// Mailbox with small-size linear insert and large-size BTreeMap per-priority buckets.
pub struct Mailbox {
    inner: MailboxImpl,
}

impl Clone for Mailbox {
    fn clone(&self) -> Self {
        Mailbox {
            inner: match &self.inner {
                MailboxImpl::Linear(v) => MailboxImpl::Linear(v.clone()),
                MailboxImpl::Map(m) => MailboxImpl::Map(m.clone()),
            },
        }
    }
}

enum MailboxImpl {
    Linear(VecDeque<core::ast::ValueEnvelope>),
    Map(BTreeMap<i32, VecDeque<core::ast::ValueEnvelope>>), // key = priority (ascending)
}

impl Default for Mailbox {
    fn default() -> Self {
        Self::new()
    }
}

impl Mailbox {
    const MIGRATE_THRESHOLD: usize = 64; // simple heuristic

    pub fn new() -> Self {
        Mailbox {
            inner: MailboxImpl::Linear(VecDeque::new()),
        }
    }

    pub fn len(&self) -> usize {
        match &self.inner {
            MailboxImpl::Linear(v) => v.len(),
            MailboxImpl::Map(m) => m.values().map(|q| q.len()).sum(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn front(&self) -> Option<&core::ast::ValueEnvelope> {
        match &self.inner {
            MailboxImpl::Linear(v) => v.front(),
            MailboxImpl::Map(m) => {
                // highest priority -> last key in BTreeMap
                m.keys()
                    .next_back()
                    .and_then(|k| m.get(k))
                    .and_then(|q| q.front())
            }
        }
    }

    pub fn to_vec(&self) -> Vec<core::ast::ValueEnvelope> {
        match &self.inner {
            MailboxImpl::Linear(v) => v.iter().cloned().collect(),
            MailboxImpl::Map(m) => {
                let mut out = Vec::new();
                for (&_pri, q) in m.iter().rev() {
                    // descending priority
                    for e in q {
                        out.push(e.clone());
                    }
                }
                out
            }
        }
    }

    pub fn pop_front(&mut self) -> Option<core::ast::ValueEnvelope> {
        match &mut self.inner {
            MailboxImpl::Linear(v) => v.pop_front(),
            MailboxImpl::Map(m) => {
                if let Some((&pri, _)) = m.iter().next_back()
                    && let Some(q) = m.get_mut(&pri)
                {
                    let res = q.pop_front();
                    if q.is_empty() {
                        m.remove(&pri);
                    }
                    return res;
                }
                None
            }
        }
    }

    pub fn insert(&mut self, env: core::ast::ValueEnvelope) {
        match &mut self.inner {
            MailboxImpl::Linear(v) => {
                // linear insert by priority with FIFO among equals
                let mut insert_pos = 0usize;
                while insert_pos < v.len() {
                    if v[insert_pos].priority < env.priority {
                        break;
                    }
                    insert_pos += 1;
                }
                while insert_pos < v.len() && v[insert_pos].priority == env.priority {
                    insert_pos += 1;
                }
                v.insert(insert_pos, env);
                if v.len() > Mailbox::MIGRATE_THRESHOLD {
                    // migrate to map
                    let mut map: BTreeMap<i32, VecDeque<core::ast::ValueEnvelope>> =
                        BTreeMap::new();
                    for e in v.drain(..) {
                        map.entry(e.priority).or_default().push_back(e);
                    }
                    self.inner = MailboxImpl::Map(map);
                }
            }
            MailboxImpl::Map(m) => {
                m.entry(env.priority).or_default().push_back(env);
            }
        }
    }

    pub fn iter(&self) -> Vec<core::ast::ValueEnvelope> {
        self.to_vec()
    }
}

pub fn encode_val(val: &ArtValue, out: &mut Vec<u8>) -> std::result::Result<(), String> {
    match val {
        ArtValue::Int(n) => {
            out.push(1);
            out.extend_from_slice(&n.to_le_bytes());
        }
        ArtValue::Float(f) => {
            out.push(2);
            out.extend_from_slice(&f.to_le_bytes());
        }
        ArtValue::String(s) => {
            out.push(3);
            out.extend_from_slice(&(s.len() as u32).to_le_bytes());
            out.extend_from_slice(s.as_bytes());
        }
        ArtValue::Bool(b) => {
            out.push(4);
            out.push(if *b { 1 } else { 0 });
        }
        ArtValue::Optional(opt) => {
            if let Some(inner) = &**opt {
                out.push(5);
                encode_val(inner, out)?;
            } else {
                out.push(6);
            }
        }
        ArtValue::Array(arr) => {
            out.push(7);
            out.extend_from_slice(&(arr.len() as u32).to_le_bytes());
            for item in arr {
                encode_val(item, out)?;
            }
        }
        ArtValue::Map(m) => {
            let map = m.0.lock().unwrap_or_else(|e| e.into_inner());
            out.push(8);
            out.extend_from_slice(&(map.len() as u32).to_le_bytes());
            for (k, v) in map.iter() {
                let k_bytes = k.as_bytes();
                out.extend_from_slice(&(k_bytes.len() as u32).to_le_bytes());
                out.extend_from_slice(k_bytes);
                encode_val(v, out)?;
            }
        }
        ArtValue::Set(s) => {
            let set = s.0.lock().unwrap_or_else(|e| e.into_inner());
            out.push(9);
            out.extend_from_slice(&(set.len() as u32).to_le_bytes());
            for item in set.iter() {
                encode_val(item, out)?;
            }
        }
        ArtValue::Buffer(buf) => {
            out.push(10);
            out.extend_from_slice(&(buf.len() as u32).to_le_bytes());
            out.extend_from_slice(buf);
        }
        ArtValue::StructInstance {
            struct_name,
            fields,
        } => {
            out.push(11);
            let n_bytes = struct_name.as_bytes();
            out.extend_from_slice(&(n_bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(n_bytes);
            out.extend_from_slice(&(fields.len() as u32).to_le_bytes());
            for (k, v) in fields {
                let k_bytes = k.as_bytes();
                out.extend_from_slice(&(k_bytes.len() as u32).to_le_bytes());
                out.extend_from_slice(k_bytes);
                encode_val(v, out)?;
            }
        }
        ArtValue::EnumInstance {
            enum_name,
            variant,
            values,
        } => {
            out.push(12);
            let n_bytes = enum_name.as_bytes();
            out.extend_from_slice(&(n_bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(n_bytes);
            let v_bytes = variant.as_bytes();
            out.extend_from_slice(&(v_bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(v_bytes);
            out.extend_from_slice(&(values.len() as u32).to_le_bytes());
            for v in values {
                encode_val(v, out)?;
            }
        }
        ArtValue::Tuple(tup) => {
            out.push(13);
            out.extend_from_slice(&(tup.len() as u32).to_le_bytes());
            for item in tup {
                encode_val(item, out)?;
            }
        }
        _ => return Err(format!("Cannot serialize type {}", val)),
    }
    Ok(())
}

pub fn decode_val(cur: &mut std::io::Cursor<&[u8]>) -> std::result::Result<ArtValue, String> {
    use std::io::Read;
    let mut tag = [0u8; 1];
    cur.read_exact(&mut tag).map_err(|_| "EOF reading tag")?;
    match tag[0] {
        1 => {
            let mut b = [0u8; 8];
            cur.read_exact(&mut b).map_err(|_| "EOF reading int")?;
            Ok(ArtValue::Int(i64::from_le_bytes(b)))
        }
        2 => {
            let mut b = [0u8; 8];
            cur.read_exact(&mut b).map_err(|_| "EOF reading float")?;
            Ok(ArtValue::Float(f64::from_le_bytes(b)))
        }
        3 => {
            let mut b = [0u8; 4];
            cur.read_exact(&mut b).map_err(|_| "EOF str len")?;
            let mut str_b = vec![0u8; u32::from_le_bytes(b) as usize];
            cur.read_exact(&mut str_b).map_err(|_| "EOF str")?;
            Ok(ArtValue::String(
                String::from_utf8(str_b).map_err(|_| "UTF8 error")?.into(),
            ))
        }
        4 => {
            let mut b = [0u8; 1];
            cur.read_exact(&mut b).map_err(|_| "EOF bool")?;
            Ok(ArtValue::Bool(b[0] != 0))
        }
        5 => Ok(ArtValue::Optional(Box::new(Some(decode_val(cur)?)))),
        6 => Ok(ArtValue::none()),
        7 => {
            let mut b = [0u8; 4];
            cur.read_exact(&mut b).map_err(|_| "EOF array len")?;
            let len = u32::from_le_bytes(b) as usize;
            let mut arr = Vec::with_capacity(len);
            for _ in 0..len {
                arr.push(decode_val(cur)?);
            }
            Ok(ArtValue::Array(arr))
        }
        8 => {
            let mut b = [0u8; 4];
            cur.read_exact(&mut b).map_err(|_| "EOF map len")?;
            let len = u32::from_le_bytes(b) as usize;
            let mut map = std::collections::HashMap::with_capacity(len);
            for _ in 0..len {
                let mut lb = [0u8; 4];
                cur.read_exact(&mut lb).map_err(|_| "EOF map k len")?;
                let mut k_b = vec![0u8; u32::from_le_bytes(lb) as usize];
                cur.read_exact(&mut k_b).map_err(|_| "EOF map k")?;
                let k_str = String::from_utf8(k_b).map_err(|_| "UTF8 error")?;
                map.insert(k_str, decode_val(cur)?);
            }
            Ok(ArtValue::Map(core::ast::MapRef(std::sync::Arc::new(
                std::sync::Mutex::new(map),
            ))))
        }
        9 => {
            let mut b = [0u8; 4];
            cur.read_exact(&mut b).map_err(|_| "EOF set len")?;
            let len = u32::from_le_bytes(b) as usize;
            let mut set = Vec::with_capacity(len);
            for _ in 0..len {
                set.push(decode_val(cur)?);
            }
            Ok(ArtValue::Set(core::ast::SetRef(std::sync::Arc::new(
                std::sync::Mutex::new(set),
            ))))
        }
        10 => {
            let mut b = [0u8; 4];
            cur.read_exact(&mut b).map_err(|_| "EOF buffer len")?;
            let mut buf = vec![0u8; u32::from_le_bytes(b) as usize];
            cur.read_exact(&mut buf).map_err(|_| "EOF buf")?;
            Ok(ArtValue::Buffer(buf.into()))
        }
        11 => {
            let mut lb = [0u8; 4];
            cur.read_exact(&mut lb).map_err(|_| "EOF struct name len")?;
            let mut sn_b = vec![0u8; u32::from_le_bytes(lb) as usize];
            cur.read_exact(&mut sn_b).map_err(|_| "EOF struct name")?;
            let struct_name = String::from_utf8(sn_b).map_err(|_| "UTF8 error")?;
            let mut fb = [0u8; 4];
            cur.read_exact(&mut fb).map_err(|_| "EOF fields len")?;
            let f_len = u32::from_le_bytes(fb) as usize;
            let mut fields = std::collections::HashMap::with_capacity(f_len);
            for _ in 0..f_len {
                let mut klb = [0u8; 4];
                cur.read_exact(&mut klb).map_err(|_| "EOF field k len")?;
                let mut k_b = vec![0u8; u32::from_le_bytes(klb) as usize];
                cur.read_exact(&mut k_b).map_err(|_| "EOF field name")?;
                let fname = String::from_utf8(k_b).map_err(|_| "UTF8 error")?;
                fields.insert(fname, decode_val(cur)?);
            }
            Ok(ArtValue::StructInstance {
                struct_name,
                fields,
            })
        }
        12 => {
            let mut lb = [0u8; 4];
            cur.read_exact(&mut lb).map_err(|_| "EOF enum name len")?;
            let mut n_b = vec![0u8; u32::from_le_bytes(lb) as usize];
            cur.read_exact(&mut n_b).map_err(|_| "EOF enum name")?;
            let enum_name = String::from_utf8(n_b).map_err(|_| "UTF8 err")?;

            let mut vb = [0u8; 4];
            cur.read_exact(&mut vb).map_err(|_| "EOF variant len")?;
            let mut v_b = vec![0u8; u32::from_le_bytes(vb) as usize];
            cur.read_exact(&mut v_b).map_err(|_| "EOF variant")?;
            let variant = String::from_utf8(v_b).map_err(|_| "UTF8 err")?;

            let mut ab = [0u8; 4];
            cur.read_exact(&mut ab).map_err(|_| "EOF arr len")?;
            let arr_len = u32::from_le_bytes(ab) as usize;
            let mut values = Vec::with_capacity(arr_len);
            for _ in 0..arr_len {
                values.push(decode_val(cur)?);
            }
            Ok(ArtValue::EnumInstance {
                enum_name,
                variant,
                values,
            })
        }
        13 => {
            let mut b = [0u8; 4];
            cur.read_exact(&mut b).map_err(|_| "EOF tuple len")?;
            let len = u32::from_le_bytes(b) as usize;
            let mut tup = Vec::with_capacity(len);
            for _ in 0..len {
                tup.push(decode_val(cur)?);
            }
            Ok(ArtValue::Tuple(tup))
        }
        t => Err(format!("Unknown tag {}", t)),
    }
}

impl Interpreter {
    pub fn run_actors_round_robin(&mut self, max_steps: usize) {
        let mut steps = 0usize;
        let mut actor_ids: Vec<u32> = self.actors.keys().cloned().collect();
        actor_ids.sort_unstable();
        let mut idx = 0usize;
        // rotation_progress = whether any actor made progress during the current full pass
        let mut rotation_progress = false;
        while steps < max_steps && !actor_ids.is_empty() {
            if idx >= actor_ids.len() {
                // completed a full pass
                if !rotation_progress {
                    // no actor made progress during the full rotation -> quiescent
                    break;
                }
                rotation_progress = false;
                idx = 0;
            }
            let aid = actor_ids[idx];
            // If actor was removed or finished, skip
            let should_remove = if let Some(actor) = self.actors.get(&aid) {
                actor.finished
            } else {
                true
            };
            if should_remove {
                // remove from list
                actor_ids.remove(idx);
                continue;
            }

            // Execute one statement of the actor if available
            if let Some(actor_entry) = self.actors.remove(&aid) {
                // Store in executing_actor during execution to allow builtins to access
                // the actor state even though it's temporarily removed from the map.
                self.executing_actor = Some(actor_entry);

                // If parked (waiting for message) skip until unparked (actor_send will unpark)
                if self
                    .executing_actor
                    .as_ref()
                    .expect("set two lines above")
                    .parked
                {
                    let actor = self.executing_actor.take().expect("set two lines above");
                    self.actors.insert(aid, actor);
                    idx += 1;
                    continue;
                }

                // Determine if actor is runnable: has body statements or mailbox with content
                let is_runnable = {
                    let act = self.executing_actor.as_ref().expect("set above");
                    !act.body.is_empty() || !act.mailbox.is_empty()
                };

                if !is_runnable {
                    // nothing to do for this actor; reinsert and skip
                    let actor = self.executing_actor.take().expect("set above");
                    self.actors.insert(aid, actor);
                    idx += 1;
                    continue;
                }

                // set current actor context
                self.current_actor = Some(aid);

                // Pop statement if available
                let stmt_opt = {
                    let act = self.executing_actor.as_mut().expect("set above");
                    act.body.pop_front()
                };

                if let Some(stmt) = stmt_opt {
                    // Swap environment
                    let previous_env = self.environment.clone();
                    self.environment = self
                        .executing_actor
                        .as_ref()
                        .expect("set above")
                        .env
                        .clone();
                    let actor_env_before_stmt = self.environment.clone();
                    // Execute statement; ignore return errors for now
                    let _ = self.execute(stmt.clone());

                    let actor_parked = self
                        .executing_actor
                        .as_ref()
                        .map(|a| a.parked)
                        .unwrap_or(false);
                    if actor_parked {
                        // actor_receive*/mailbox wait semantics: retry the same statement
                        // after unpark so local bindings are evaluated with a real message.
                        if let Some(act) = self.executing_actor.as_mut() {
                            act.body.push_front(stmt);
                        }
                        self.environment = actor_env_before_stmt;
                    }

                    // Mark that we made progress this rotation (executed a statement)
                    rotation_progress = true;
                    // restore env
                    if let Some(act) = &mut self.executing_actor {
                        act.env = self.environment.clone();
                    }
                    self.environment = previous_env;
                } else {
                    // No statements; actor may be waiting for mailbox messages handled by actor_receive
                    // nothing to step here
                }
                // Clear current actor context
                self.current_actor = None;

                // Take actor back
                if let Some(mut actor) = self.executing_actor.take() {
                    // If actor has no body and mailbox empty, mark finished
                    if actor.body.is_empty() && actor.mailbox.is_empty() {
                        actor.finished = true;
                    }
                    // reinsert actor state
                    self.actors.insert(aid, actor);
                }
            }

            steps += 1;
            idx += 1;
        }
        // Cleanup finished actors
        let finished_ids: Vec<u32> = self
            .actors
            .iter()
            .filter_map(|(id, a)| if a.finished { Some(*id) } else { None })
            .collect();
        for id in finished_ids {
            self.actors.remove(&id);
        }
    }
}
