/// Source map V3 builder.
///
/// Produces a JSON source map with VLQ-encoded `mappings` field.
/// Spec: https://docs.google.com/document/d/1U1RGAehQwRypUTovF1KRlpiOFze0b-_2gc6fAH0KY0k
const BASE64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn encode_vlq(n: i32) -> String {
    let mut v = if n < 0 { ((-n) << 1) | 1 } else { n << 1 };
    let mut out = String::new();
    loop {
        let mut digit = v & 0x1F;
        v >>= 5;
        if v > 0 {
            digit |= 0x20;
        }
        out.push(BASE64[digit as usize] as char);
        if v == 0 {
            break;
        }
    }
    out
}

#[derive(Default)]
pub struct SourceMapBuilder {
    /// (gen_line, gen_col, src_line, src_col) — 0-based
    mappings: Vec<(u32, u32, u32, u32)>,
}

impl SourceMapBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, gen_line: u32, gen_col: u32, src_line: u32, src_col: u32) {
        self.mappings.push((gen_line, gen_col, src_line, src_col));
    }

    pub fn build(&self, source_file: &str, source_content: Option<&str>) -> String {
        let mappings = self.encode_mappings();
        let sources_content = match source_content {
            Some(c) => format!(
                ",\"sourcesContent\":[{}]",
                serde_json_string(c)
            ),
            None => String::new(),
        };
        format!(
            "{{\"version\":3,\"file\":\"\",\"sourceRoot\":\"\",\"sources\":[{}],\"names\":[],\"mappings\":\"{}\"{}}}",
            serde_json_string(source_file),
            mappings,
            sources_content,
        )
    }

    fn encode_mappings(&self) -> String {
        if self.mappings.is_empty() {
            return String::new();
        }

        // Sort by generated position
        let mut sorted = self.mappings.clone();
        sorted.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

        let max_line = sorted.last().map(|m| m.0).unwrap_or(0);
        let mut result = String::new();
        let mut prev_src_line: i32 = 0;
        let mut prev_src_col: i32 = 0;
        let mut mapping_idx = 0;

        for gen_line in 0..=max_line {
            if gen_line > 0 {
                result.push(';');
            }
            let mut prev_gen_col: i32 = 0;
            let mut first_seg = true;

            while mapping_idx < sorted.len() && sorted[mapping_idx].0 == gen_line {
                let (_, gen_col, src_line, src_col) = sorted[mapping_idx];
                if !first_seg {
                    result.push(',');
                }
                first_seg = false;

                // Segment: [gen_col_delta, src_index=0, src_line_delta, src_col_delta]
                result.push_str(&encode_vlq(gen_col as i32 - prev_gen_col));
                result.push_str(&encode_vlq(0)); // source index always 0
                result.push_str(&encode_vlq(src_line as i32 - prev_src_line));
                result.push_str(&encode_vlq(src_col as i32 - prev_src_col));

                prev_gen_col = gen_col as i32;
                prev_src_line = src_line as i32;
                prev_src_col = src_col as i32;
                mapping_idx += 1;
            }
        }
        result
    }
}

fn serde_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vlq_zero() {
        assert_eq!(encode_vlq(0), "A");
    }

    #[test]
    fn vlq_positive() {
        assert_eq!(encode_vlq(1), "C");
        assert_eq!(encode_vlq(16), "gB");
    }

    #[test]
    fn vlq_negative() {
        assert_eq!(encode_vlq(-1), "D");
    }

    #[test]
    fn sourcemap_basic() {
        let mut b = SourceMapBuilder::new();
        b.add(0, 0, 0, 0);
        let json = b.build("test.art", None);
        assert!(json.contains("\"version\":3"));
        assert!(json.contains("test.art"));
    }
}
