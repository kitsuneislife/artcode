use crate::interpreter::encode_val;
use core::ast::{ArtValue, MapRef};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::sync::{Arc, Mutex};

// Format: 8-byte magic header
// Events: Delta stream + optional checkpoint markers (keyframes)
pub struct Tracer {
    file: File,
    /// last full snapshot used to compute deltas
    last_snapshot: Option<HashMap<String, ArtValue>>,
}

impl Tracer {
    pub fn new(path: &str) -> std::io::Result<Self> {
        let mut file = File::create(path)?;
        file.write_all(b"ARTLOG01")?;
        Ok(Self {
            file,
            last_snapshot: None,
        })
    }

    /// Grava um evento interceptado gerando um Record persistente (Event Sourcing)
    pub fn record_event(
        &mut self,
        event_type: &str,
        tick: usize,
        payload: ArtValue,
    ) -> Result<(), String> {
        let mut map = std::collections::HashMap::new();
        map.insert("type".to_string(), ArtValue::String(event_type.into()));
        map.insert("tick".to_string(), ArtValue::Int(tick as i64));
        map.insert("payload".to_string(), payload);

        let event = ArtValue::Map(MapRef(Arc::new(std::sync::Mutex::new(map))));

        // Usa o binario zero-copy IPC
        let mut buffer = Vec::new();
        encode_val(&event, &mut buffer).map_err(|e| format!("Serialization error: {}", e))?;

        let size = buffer.len() as u32;
        let p_size = size.to_le_bytes();
        if let Err(e) = self.file.write_all(&p_size) {
            return Err(e.to_string());
        }
        if let Err(e) = self.file.write_all(&buffer) {
            return Err(e.to_string());
        }

        let _ = self.file.flush();
        Ok(())
    }

    /// Record only the changed keys since the last snapshot (delta encoding).
    /// Falls back to a full keyframe if no previous snapshot exists.
    /// Returns the number of changed keys (0 = identical to last snapshot → event skipped).
    pub fn record_delta(
        &mut self,
        tick: usize,
        snapshot: HashMap<String, ArtValue>,
    ) -> Result<usize, String> {
        let changed: HashMap<String, ArtValue> = match &self.last_snapshot {
            None => snapshot.clone(),
            Some(prev) => snapshot
                .iter()
                .filter(|(k, v)| prev.get(*k) != Some(v))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        };

        if changed.is_empty() {
            self.last_snapshot = Some(snapshot);
            return Ok(0);
        }

        let count = changed.len();
        let mut payload_map = HashMap::new();
        payload_map.insert("tick".to_string(), ArtValue::Int(tick as i64));
        payload_map.insert(
            "changed".to_string(),
            ArtValue::Map(MapRef(Arc::new(Mutex::new(changed)))),
        );

        let event = ArtValue::Map(MapRef(Arc::new(Mutex::new(payload_map))));
        let mut buffer = Vec::new();
        encode_val(&event, &mut buffer).map_err(|e| format!("Serialization error: {}", e))?;
        let size = (buffer.len() as u32).to_le_bytes();
        self.file.write_all(&size).map_err(|e| e.to_string())?;
        self.file.write_all(&buffer).map_err(|e| e.to_string())?;
        let _ = self.file.flush();
        self.last_snapshot = Some(snapshot);
        Ok(count)
    }

    pub fn record_checkpoint(&mut self, tick: usize, rng_state: u64) -> Result<(), String> {
        let mut payload = std::collections::HashMap::new();
        payload.insert(
            "rng_state".to_string(),
            ArtValue::String(rng_state.to_string().into()),
        );

        let checkpoint_payload = ArtValue::Map(MapRef(Arc::new(Mutex::new(payload))));
        self.record_event("checkpoint", tick, checkpoint_payload)
    }
}
