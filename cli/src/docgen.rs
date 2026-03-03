use std::fs;
use std::io;
use std::path::Path;

struct DocItem {
    kind: String, // "Function", "Struct", "Variable", "Module"
    signature: String,
    docstring: String,
}

pub fn generate_html<P: AsRef<Path>>(input_path: P) -> io::Result<()> {
    let content = fs::read_to_string(&input_path)?;
    let items = parse_doc_items(&content);

    let html = build_html_document(input_path.as_ref().to_str().unwrap_or("Module"), &items);

    let out_path = "docs.html";
    fs::write(out_path, html)?;

    println!("Generated HTML documentation at {}", out_path);
    Ok(())
}

fn parse_doc_items(content: &str) -> Vec<DocItem> {
    let mut items = Vec::new();
    let mut current_docs = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("//") {
            let doc_text = trimmed.strip_prefix("//").unwrap().trim();
            current_docs.push(doc_text.to_string());
        } else if trimmed.starts_with("func ") {
            // function signature up to '{'
            let sig = trimmed.split('{').next().unwrap_or(trimmed).trim();
            items.push(DocItem {
                kind: "Function".to_string(),
                signature: sig.to_string(),
                docstring: current_docs.join(" "),
            });
            current_docs.clear();
        } else if trimmed.starts_with("struct ") {
            let sig = trimmed.split('{').next().unwrap_or(trimmed).trim();
            items.push(DocItem {
                kind: "Struct".to_string(),
                signature: sig.to_string(),
                docstring: current_docs.join(" "),
            });
            current_docs.clear();
        } else if trimmed.starts_with("enum ") {
            let sig = trimmed.split('{').next().unwrap_or(trimmed).trim();
            items.push(DocItem {
                kind: "Enum".to_string(),
                signature: sig.to_string(),
                docstring: current_docs.join(" "),
            });
            current_docs.clear();
        } else if trimmed.starts_with("let ") && !current_docs.is_empty() {
            // only capture documented lets
            let sig = trimmed.split('=').next().unwrap_or(trimmed).trim();
            items.push(DocItem {
                kind: "Variable".to_string(),
                signature: sig.to_string(),
                docstring: current_docs.join(" "),
            });
            current_docs.clear();
        } else if trimmed.is_empty() {
            // Keep aggregating docs across empty lines unless it feels like an explicit break
        } else {
            // It's some random code, reset docs
            current_docs.clear();
        }
    }

    items
}

fn build_html_document(title: &str, items: &[DocItem]) -> String {
    let mut html = String::new();

    // Header & Styles
    html.push_str(&format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{} API Reference</title>
    <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;600;700&display=swap" rel="stylesheet">
    <style>
        :root {{
            --bg-color: #f7f9fc;
            --text-color: #1a202c;
            --card-bg: rgba(255, 255, 255, 0.8);
            --border-color: rgba(226, 232, 240, 0.8);
            --primary: #3182ce;
            --tag-bg: #edf2f7;
            --tag-text: #4a5568;
        }}
        @media (prefers-color-scheme: dark) {{
            :root {{
                --bg-color: #1a202c;
                --text-color: #f7fafc;
                --card-bg: rgba(45, 55, 72, 0.5);
                --border-color: rgba(74, 85, 104, 0.5);
                --primary: #63b3ed;
                --tag-bg: #2d3748;
                --tag-text: #cbd5e0;
            }}
        }}
        body {{
            font-family: 'Inter', -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif;
            background-color: var(--bg-color);
            color: var(--text-color);
            line-height: 1.6;
            margin: 0;
            padding: 0;
            transition: background-color 0.3s, color 0.3s;
        }}
        .container {{
            max-width: 900px;
            margin: 0 auto;
            padding: 2rem 1rem;
        }}
        header {{
            text-align: center;
            margin-bottom: 3rem;
        }}
        h1 {{
            font-size: 2.5rem;
            margin-bottom: 0.5rem;
            font-weight: 700;
        }}
        .title-subtitle {{
            color: var(--primary);
            font-size: 1.1rem;
            opacity: 0.9;
        }}
        .item-card {{
            background: var(--card-bg);
            border: 1px solid var(--border-color);
            border-radius: 12px;
            padding: 1.5rem;
            margin-bottom: 1.5rem;
            box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.05), 0 2px 4px -1px rgba(0, 0, 0, 0.03);
            backdrop-filter: blur(10px);
            -webkit-backdrop-filter: blur(10px);
            transition: transform 0.2s, box-shadow 0.2s;
        }}
        .item-card:hover {{
            transform: translateY(-2px);
            box-shadow: 0 10px 15px -3px rgba(0, 0, 0, 0.08), 0 4px 6px -2px rgba(0, 0, 0, 0.04);
        }}
        .item-header {{
            display: flex;
            align-items: baseline;
            gap: 1rem;
            margin-bottom: 1rem;
            padding-bottom: 0.75rem;
            border-bottom: 1px solid var(--border-color);
        }}
        .item-kind {{
            font-size: 0.75rem;
            font-weight: 700;
            text-transform: uppercase;
            letter-spacing: 0.05em;
            background-color: var(--tag-bg);
            color: var(--tag-text);
            padding: 0.25rem 0.6rem;
            border-radius: 9999px;
            flex-shrink: 0;
        }}
        .item-signature {{
            font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
            font-size: 1.1rem;
            font-weight: 600;
            color: var(--primary);
            word-break: break-all;
        }}
        .item-docstring {{
            font-size: 1rem;
            opacity: 0.9;
        }}
        .no-docs {{
            font-style: italic;
            opacity: 0.6;
        }}
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>API Reference</h1>
            <div class="title-subtitle">{}</div>
        </header>

"#, title, title));

    // Body
    if items.is_empty() {
        html.push_str("<p style='text-align: center; font-style: italic;'>No documented artifacts found in this module.</p>\n");
    } else {
        for item in items {
            let doc_html = if item.docstring.is_empty() {
                "<p class='item-docstring no-docs'>No documentation provided.</p>".to_string()
            } else {
                format!("<p class='item-docstring'>{}</p>", item.docstring)
            };

            html.push_str(&format!(
                r#"
        <div class="item-card">
            <div class="item-header">
                <span class="item-kind">{}</span>
                <span class="item-signature">{}</span>
            </div>
            {}
        </div>
"#,
                item.kind, item.signature, doc_html
            ));
        }
    }

    // Footer
    html.push_str(
        r#"
    </div>
</body>
</html>
"#,
    );

    html
}
