use std::fs;
use std::io;
use std::path::Path;

pub fn format_file<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let content = fs::read_to_string(&path)?;
    let formatted = format_string(&content);

    if content != formatted {
        fs::write(path, formatted)?;
        println!("Reformatted.");
    } else {
        println!("Already formatted.");
    }

    Ok(())
}

pub fn format_string(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut indent_level = 0;
    let indent_str = "    "; // 4 spaces

    for line in input.lines() {
        let trimmed = line.trim();

        // If line is empty, just preserve the newline
        if trimmed.is_empty() {
            out.push('\n');
            continue;
        }

        // Check if the line closes a block to decrement indent before writing
        // This is a naive heuristic for minimal idempotency.
        let mut local_indent = indent_level;
        if trimmed.starts_with('}') || trimmed.starts_with(']') || trimmed.starts_with(')') {
            if local_indent > 0 {
                local_indent -= 1;
            }
        } else if trimmed.starts_with("else") || trimmed.starts_with("case ") {
            if local_indent > 0 {
                local_indent -= 1;
            }
        }

        // Apply indentation
        for _ in 0..local_indent {
            out.push_str(indent_str);
        }

        out.push_str(trimmed);
        out.push('\n');

        // Adjust global indent level for the next lines based on chars
        for c in trimmed.chars() {
            match c {
                '{' | '[' | '(' => indent_level += 1,
                '}' | ']' | ')' => {
                    if indent_level > 0 {
                        indent_level -= 1;
                    }
                }
                _ => {}
            }
        }
    }

    // Ensure file ends with exactly one newline
    while out.ends_with("\n\n") {
        out.pop();
    }

    out
}
