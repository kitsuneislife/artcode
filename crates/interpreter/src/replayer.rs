use core::ast::ArtValue;
use crate::interpreter::decode_val;
use std::collections::VecDeque;
use std::fs::File;
use std::io::Read;

pub struct Replayer {
    // Fila pre-carregada de Eventos, decodificados
    // Cada evento tem: "type": String, "tick": i64, "payload": ArtValue
    events: VecDeque<ArtValue>,
}

impl Replayer {
    pub fn new(path: &str) -> std::io::Result<Self> {
        let mut file = File::open(path)?;
        
        let mut magic = [0u8; 8];
        file.read_exact(&mut magic)?;
        if &magic != b"ARTLOG01" {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Not a valid ARTLOG01 file",
            ));
        }

        let mut events = VecDeque::new();
        loop {
            let mut p_size = [0u8; 4];
            if file.read_exact(&mut p_size).is_err() {
                break; // EOF
            }
            let size = u32::from_le_bytes(p_size);
            
            let mut buffer = vec![0u8; size as usize];
            file.read_exact(&mut buffer)?;
            
            let mut cur = std::io::Cursor::new(buffer.as_slice());
            if let Ok(val) = decode_val(&mut cur) {
                events.push_back(val);
            }
        }
        
        Ok(Self { events })
    }

    /// Consome o próximo evento de Replay, checando o Tick e Event Type.
    /// Retorna `Some(payload)` se o checkpoint bater, consumindo da fila.
    pub fn consume_intercept(&mut self, expected_type: &str, current_tick: usize) -> Result<Option<ArtValue>, String> {
        if let Some(event) = self.events.front() {
            if let ArtValue::Map(mapref) = event {
                let map = mapref.0.lock().unwrap();
                
                let e_type = match map.get("type") {
                    Some(ArtValue::String(s)) => s.as_ref(),
                    _ => "",
                };
                let e_tick = match map.get("tick") {
                    Some(ArtValue::Int(t)) => *t as usize,
                    _ => 0,
                };
                
                // Verificacao rigida de consistencia: se o programa tentou abrir um evento que não é
                // o próximo da lista de gravação, então o fluxo do script divirgiu do logado.
                // Mas permitiremos ler apenas se o tick exato chegou e o evento é da mesma origem.
                if e_tick == current_tick && e_type == expected_type {
                    let payload = map.get("payload").cloned().unwrap_or_else(ArtValue::none);
                    // Drop mutex e pop the event from queue
                    drop(map);
                    self.events.pop_front();
                    return Ok(Some(payload));
                }
            }
        }
        Ok(None)
    }
}
