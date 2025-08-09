use core::ast::{ArtValue, InterpolatedPart, Expr};
use crate::values::Result;

pub fn eval_fstring(parts: Vec<InterpolatedPart>, mut eval: impl FnMut(Expr) -> Result<ArtValue>) -> Result<ArtValue> {
    let cap: usize = parts.iter().map(|p| match p { InterpolatedPart::Literal(s) => s.len(), InterpolatedPart::Expr { .. } => 4 }).sum();
    let mut result = String::with_capacity(cap);
    for part in parts {
        match part {
            InterpolatedPart::Literal(s) => result.push_str(&s),
            InterpolatedPart::Expr { expr, format } => {
                let val = eval(*expr)?;
                let mut seg = val.to_string();
                if let Some(spec) = format {
                    match spec.as_str() {
                        "upper" => seg = seg.to_uppercase(),
                        "lower" => seg = seg.to_lowercase(),
                        "trim" => seg = seg.trim().to_string(),
                        "debug" => { seg = format!("{:?}", val.clone()); }
                        s if s.starts_with("pad") => {
                            if let Ok(width) = s[3..].parse::<usize>() && seg.len() < width {
                                seg = format!("{:<width$}", seg, width=width);
                            }
                        }
                        "hex" => {
                            if let ArtValue::Int(n) = val { seg = format!("0x{:x}", n); }
                        }
                        _ => { /* desconhecido: ignorar */ }
                    }
                }
                result.push_str(&seg);
            }
        }
    }
    Ok(ArtValue::String(std::sync::Arc::from(result)))
}
