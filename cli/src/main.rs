use interpreter::interpreter::Interpreter;
use lexer::lexer::Lexer;
use parser::parser::Parser;
use diagnostics::format_diagnostic;
use std::fs;
use std::io::{self, Write};
use std::process;
use std::env;

fn run_with_source(_name:&str, source:String) {
    let mut lexer = Lexer::new(source.clone());
    let tokens = match lexer.scan_tokens() { Ok(t)=>t, Err(d)=>{ eprintln!("{}", format_diagnostic(&source,&d)); return; } };
    let mut parser = Parser::new(tokens);
    let (program, diags) = parser.parse();
    if !diags.is_empty() { for d in &diags { eprintln!("{}", format_diagnostic(&source,d)); } return; }
    let mut interpreter = Interpreter::with_prelude();
    if let Err(e) = interpreter.interpret(program) { eprintln!("Erro de execução: {}", e); }
    for d in interpreter.take_diagnostics() { eprintln!("{}", format_diagnostic(&source, &d)); }
    let total = interpreter.executed_statements.max(1);
    let percent = 100.0 * (1.0 - (interpreter.handled_errors as f64 / total as f64));
    eprintln!("[metrics] handled_errors={} executed_statements={} crash_free={:.1}%", interpreter.handled_errors, interpreter.executed_statements, percent);
    eprintln!("[mem] weak_created={} weak_upgrades={} weak_dangling={} unowned_created={} unowned_dangling={} cycle_reports_run={}",
        interpreter.weak_created, interpreter.weak_upgrades, interpreter.weak_dangling,
        interpreter.unowned_created, interpreter.unowned_dangling, interpreter.cycle_reports_run.get());
    if !source.trim().ends_with(";") { if let Some(val) = interpreter.last_value { println!("=> {}", val); } }
}

fn run_file(path:&str) { match fs::read_to_string(path) { Ok(src)=>run_with_source(path,src), Err(e)=>{ eprintln!("Error reading file: {}", e); process::exit(74);} } }

fn run_prompt() {
    loop {
        print!("> ");
        io::stdout().flush().ok();
        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() || line.trim().is_empty() { break; }
        run_with_source("<repl>", line);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len()==1 { return run_prompt(); }
    if args[1]=="run" && args.len()==3 { return run_file(&args[2]); }
    if args[1]=="detect-cycles" {
        let mut json=false; let mut json_pretty=false; let mut file:Option<&str>=None;
        for a in &args[2..] { match a.as_str() { "--json" => json=true, "--json-pretty" => json_pretty=true, _ => file=Some(a) } }
        if json_pretty { json=true; }
        let Some(f)=file else { println!("Usage: art detect-cycles [--json|--json-pretty] <script>"); process::exit(64); };
        match fs::read_to_string(f) {
            Ok(source) => {
                let mut lexer = Lexer::new(source.clone());
                let tokens = match lexer.scan_tokens() { Ok(t)=>t, Err(d)=>{ eprintln!("{}", format_diagnostic(&source,&d)); return; } };
                let mut parser = Parser::new(tokens);
                let (program, diags) = parser.parse();
                if !diags.is_empty() { for d in &diags { eprintln!("{}", format_diagnostic(&source,d)); } return; }
                let mut interp = Interpreter::with_prelude();
                if let Err(e) = interp.interpret(program) { eprintln!("Erro de execução: {}", e); }
                if json { println!("{}", if json_pretty { interp.detect_cycles_json_pretty() } else { interp.detect_cycles_json() }); }
                else {
                    let summary = interp.cycle_report();
                    let det = interp.detect_cycles();
                    println!("cycle_summary: weak_total={} weak_alive={} weak_dead={} unowned_total={} unowned_dangling={} cycle_leaks_detected={}", summary.weak_total, summary.weak_alive, summary.weak_dead, summary.unowned_total, summary.unowned_dangling, interp.cycle_leaks_detected);
                    if !det.cycles.is_empty() { println!("cycles:"); for info in det.cycles { println!("  - nodes={:?} isolated={} reachable_root={} leak_candidate={} suggestions={:?} ranked={:?}", info.nodes, info.isolated, info.reachable_from_root, info.leak_candidate, info.suggested_break_edges, info.ranked_suggestions); } } else { println!("cycles: none"); }
                    if !det.weak_dead.is_empty() { println!("weak_dead_ids: {:?}", det.weak_dead); }
                    if !det.unowned_dangling.is_empty() { println!("unowned_dangling_ids: {:?}", det.unowned_dangling); }
                }
            }
            Err(e) => { eprintln!("Error reading file: {}", e); process::exit(74); }
        }
        return;
    }
    println!("Usage: art [run|detect-cycles] [--json] <script>");
    process::exit(64);
}