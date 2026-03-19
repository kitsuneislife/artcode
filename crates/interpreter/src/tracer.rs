use core::ast::{ArtValue, MapRef};
use std::fs::File;
use std::io::Write;
use std::sync::Arc;
use crate::interpreter::encode_val;

// Format: 8-byte magic header 
// Events: Delta stream
pub struct Tracer {
    file: File,
}

impl Tracer {
    pub fn new(path: &str) -> std::io::Result<Self> {
        let mut file = File::create(path)?;
        file.write_all(b"ARTLOG01")?;
        Ok(Self { file })
    }

    /// Grava um evento interceptado gerando um Record persistente (Event Sourcing)
    pub fn record_event(&mut self, event_type: &str, tick: usize, payload: ArtValue) -> Result<(), String> {
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
}
