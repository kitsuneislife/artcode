use core::ast::{Expr, TemplateAttrValue, TemplateNode};
use lexer::Lexer;
use parser::Parser;

fn parse_expr(src: &str) -> Expr {
    let tokens = Lexer::new(src.to_string()).scan_tokens().expect("lex");
    let mut parser = Parser::new(tokens);
    parser.expression()
}

fn parse(src: &str) -> (Vec<core::ast::Stmt>, Vec<diagnostics::Diagnostic>) {
    let tokens = Lexer::new(src.to_string()).scan_tokens().expect("lex");
    Parser::new(tokens).parse()
}

#[test]
fn test_simple_element_no_children() {
    let expr = parse_expr("<div />");
    match expr {
        Expr::Template(nodes) => {
            assert_eq!(nodes.len(), 1);
            assert!(matches!(&nodes[0], TemplateNode::Element { tag, attrs, children }
                if tag == "div" && attrs.is_empty() && children.is_empty()));
        }
        other => panic!("Expected Template, got {:?}", other),
    }
}

#[test]
fn test_element_with_static_attr() {
    let expr = parse_expr(r#"<div class="container" />"#);
    match expr {
        Expr::Template(nodes) => {
            let node = &nodes[0];
            if let TemplateNode::Element { tag, attrs, .. } = node {
                assert_eq!(tag, "div");
                assert_eq!(attrs.len(), 1);
                assert_eq!(attrs[0].name, "class");
                assert!(matches!(&attrs[0].value, TemplateAttrValue::Static(s) if s == "container"));
            } else {
                panic!("Expected Element");
            }
        }
        _ => panic!("Expected Template"),
    }
}

#[test]
fn test_element_with_dynamic_attr() {
    let expr = parse_expr("<input value={count} />");
    match expr {
        Expr::Template(nodes) => {
            if let TemplateNode::Element { attrs, .. } = &nodes[0] {
                assert_eq!(attrs.len(), 1);
                assert_eq!(attrs[0].name, "value");
                assert!(matches!(&attrs[0].value, TemplateAttrValue::Dynamic(_)));
            } else {
                panic!("Expected Element");
            }
        }
        _ => panic!("Expected Template"),
    }
}

#[test]
fn test_element_with_event_handler() {
    let expr = parse_expr("<button on:click={count} />");
    match expr {
        Expr::Template(nodes) => {
            if let TemplateNode::Element { attrs, .. } = &nodes[0] {
                let handler = attrs.iter().find(|a| a.name == "on:click");
                assert!(handler.is_some());
                assert!(matches!(&handler.unwrap().value, TemplateAttrValue::EventHandler(_)));
            } else {
                panic!("Expected Element");
            }
        }
        _ => panic!("Expected Template"),
    }
}

#[test]
fn test_element_with_expr_child() {
    let expr = parse_expr("<h1>{title}</h1>");
    match expr {
        Expr::Template(nodes) => {
            if let TemplateNode::Element { tag, children, .. } = &nodes[0] {
                assert_eq!(tag, "h1");
                assert_eq!(children.len(), 1);
                assert!(matches!(&children[0], TemplateNode::Expr(_)));
            } else {
                panic!("Expected Element");
            }
        }
        _ => panic!("Expected Template"),
    }
}

#[test]
fn test_component_is_detected_by_uppercase() {
    let expr = parse_expr("<Counter />");
    match expr {
        Expr::Template(nodes) => {
            assert!(matches!(&nodes[0], TemplateNode::Component { name, .. } if name == "Counter"));
        }
        _ => panic!("Expected Template"),
    }
}

#[test]
fn test_if_node_basic() {
    let expr = parse_expr("<if cond={x}><div /></if>");
    match expr {
        Expr::Template(nodes) => {
            assert!(matches!(&nodes[0], TemplateNode::If { .. }));
            if let TemplateNode::If { then_children, else_children, .. } = &nodes[0] {
                assert_eq!(then_children.len(), 1);
                assert!(else_children.is_empty());
            }
        }
        _ => panic!("Expected Template"),
    }
}

#[test]
fn test_if_else_node() {
    let expr = parse_expr("<if cond={ok}><span /></if><else><div /></else>");
    match expr {
        Expr::Template(nodes) => {
            assert!(matches!(&nodes[0], TemplateNode::If { .. }));
            if let TemplateNode::If { else_children, .. } = &nodes[0] {
                assert_eq!(else_children.len(), 1);
            }
        }
        _ => panic!("Expected Template"),
    }
}

#[test]
fn test_slot_node() {
    let expr = parse_expr(r#"<slot name="header" />"#);
    match expr {
        Expr::Template(nodes) => {
            assert!(matches!(&nodes[0], TemplateNode::Slot { name: Some(n), .. } if n == "header"));
        }
        _ => panic!("Expected Template"),
    }
}

#[test]
fn test_unclosed_tag_emits_diagnostic() {
    let src = "let x = <div>";
    let (_, diags) = parse(src);
    assert!(!diags.is_empty(), "Expected diagnostic for unclosed tag");
    let msg = &diags[0].message;
    assert!(
        msg.contains("Unclosed") || msg.contains("Expect '>'") || msg.contains("Expected"),
        "Unexpected diagnostic message: {}",
        msg
    );
}

#[test]
fn test_for_without_key_emits_warning() {
    // <for item in {items}><div /></for> should warn about missing key
    let src = "let t = <for item in {items}><div /></for>";
    let (_, diags) = parse(src);
    let has_key_warning = diags.iter().any(|d| d.message.contains("key"));
    assert!(has_key_warning, "Expected warning about missing key in <for>");
}

#[test]
fn test_template_in_let_binding() {
    let src = r#"let ui = <div class="app">{name}</div>"#;
    let (stmts, diags) = parse(src);
    assert!(diags.is_empty(), "Unexpected diagnostics: {:?}", diags);
    assert_eq!(stmts.len(), 1);
}

#[test]
fn test_component_without_import_emits_error() {
    let src = r#"let ui = <Counter label="x" />"#;
    let (_, diags) = parse(src);
    let has_import_error = diags
        .iter()
        .any(|d| d.message.contains("Counter") && d.message.contains("not imported"));
    assert!(has_import_error, "Expected error for undefined component 'Counter'. Got: {:?}", diags);
}

#[test]
fn test_component_with_struct_no_error() {
    // If a struct named Counter is defined, the component should be allowed.
    let src = "struct Counter { label: String }\nlet view = <Counter label=\"x\" />";
    let (_, diags) = parse(src);
    let has_component_error = diags
        .iter()
        .any(|d| d.message.contains("Counter") && d.message.contains("not imported"));
    assert!(!has_component_error, "Should not error when struct is defined. Got: {:?}", diags);
}
