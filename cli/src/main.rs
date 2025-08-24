use diagnostics::format_diagnostic;
mod resolver;
use interpreter::interpreter::Interpreter;
use interpreter::type_infer::{TypeEnv, TypeInfer};
use lexer::lexer::Lexer;
use parser::parser::Parser;
use serde::Serialize;
use toml::Value as TomlValue;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::process;

fn run_with_source(_name: &str, source: String) {
    let mut lexer = Lexer::new(source.clone());
    let tokens = match lexer.scan_tokens() {
        Ok(t) => t,
        Err(d) => {
            eprintln!("{}", format_diagnostic(&source, &d));
            return;
        }
    };
    let mut parser = Parser::new(tokens);
    let (program, diags) = parser.parse();
    if !diags.is_empty() {
        for d in &diags {
            eprintln!("{}", format_diagnostic(&source, d));
        }
        return;
    }
    // Run conservative type inference/static checks and abort on type diagnostics.
    let mut tenv = TypeEnv::new();
    let mut tinf = TypeInfer::new(&mut tenv);
    if let Err(type_diags) = tinf.run(&program) {
        for d in &type_diags {
            eprintln!("{}", format_diagnostic(&source, d));
        }
        return;
    }
    let mut interpreter = Interpreter::with_prelude();
    if let Err(e) = interpreter.interpret(program) {
        eprintln!("Erro de execução: {}", e);
    }
    for d in interpreter.take_diagnostics() {
        eprintln!("{}", format_diagnostic(&source, &d));
    }
    let total = interpreter.executed_statements.max(1);
    let percent = 100.0 * (1.0 - (interpreter.handled_errors as f64 / total as f64));
    eprintln!(
        "[metrics] handled_errors={} executed_statements={} crash_free={:.1}%",
        interpreter.handled_errors, interpreter.executed_statements, percent
    );
    eprintln!("[mem] weak_created={} weak_upgrades={} weak_dangling={} unowned_created={} unowned_dangling={} cycle_reports_run={}",
        interpreter.weak_created, interpreter.weak_upgrades, interpreter.weak_dangling,
        interpreter.unowned_created, interpreter.unowned_dangling, interpreter.cycle_reports_run.get());
    if !source.trim().ends_with(";") {
        if let Some(val) = interpreter.last_value {
            println!("=> {}", val);
        }
    }
}

fn run_file(path: &str) {
    // Use resolver to expand imports
    match crate::resolver::resolve(path) {
        Ok((program, main_source)) => {
            // We have a flattened program; run type-infer and interpreter on it
            let mut tenv = TypeEnv::new();
            let mut tinf = TypeInfer::new(&mut tenv);
            if let Err(type_diags) = tinf.run(&program) {
                for d in &type_diags {
                    eprintln!("{}", format_diagnostic(&main_source, d));
                }
                return;
            }
            let mut interpreter = Interpreter::with_prelude();
            if let Err(e) = interpreter.interpret(program) {
                eprintln!("Erro de execução: {}", e);
            }
            for d in interpreter.take_diagnostics() {
                eprintln!("{}", format_diagnostic(&main_source, &d));
            }
            return;
        }
        Err(diags) => {
            for (src, d) in diags {
                eprintln!("{}", format_diagnostic(&src, &d));
            }
            process::exit(65);
        }
    }
}

fn run_prompt() {
    loop {
        print!("> ");
        io::stdout().flush().ok();
        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() || line.trim().is_empty() {
            break;
        }
        run_with_source("<repl>", line);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() == 1 {
        return run_prompt();
    }
    if args[1] == "run" && args.len() == 3 {
        return run_file(&args[2]);
    }
    if args[1] == "metrics" {
        if args.len() < 3 {
            println!("Usage: art metrics [--json] <script>");
            process::exit(64);
        }
        let mut json = false;
        let mut file: Option<&str> = None;
        for a in &args[2..] {
            match a.as_str() {
                "--json" => json = true,
                other => file = Some(other),
            }
        }
        let Some(f) = file else {
            println!("Usage: art metrics [--json] <script>");
            process::exit(64);
        };
        match fs::read_to_string(f) {
        Ok(_source) => {
                // Use resolver to expand imports before collecting metrics
                match crate::resolver::resolve(f) {
                    Ok((program, main_source)) => {
                        // Run type inference/static checks before interpretation in metrics mode as well.
                        let mut tenv = TypeEnv::new();
                        let mut tinf = TypeInfer::new(&mut tenv);
                        if let Err(type_diags) = tinf.run(&program) {
                            for d in &type_diags {
                                eprintln!("{}", format_diagnostic(&main_source, d));
                            }
                            return;
                        }
                        let mut interpreter = Interpreter::with_prelude();
                        // habilitar checagens de invariantes por padrão ao coletar métricas
                        interpreter.enable_invariant_checks(true);
                        if let Err(e) = interpreter.interpret(program) {
                            eprintln!("Erro de execução: {}", e);
                        }
                        for d in interpreter.take_diagnostics() {
                            eprintln!("{}", format_diagnostic(&main_source, &d));
                        }

                        // Print metrics (JSON or plain) while `interpreter` is in scope.
                        if json {
                            #[derive(Serialize)]
                            struct Metrics {
                                handled_errors: usize,
                                executed_statements: usize,
                                crash_free: f64,
                                finalizer_promotions: usize,
                                objects_finalized_per_arena: std::collections::HashMap<u32, usize>,
                                arena_alloc_count: std::collections::HashMap<u32, usize>,
                                finalizer_promotions_per_arena: std::collections::HashMap<u32, usize>,
                                weak_created: usize,
                                weak_upgrades: usize,
                                weak_dangling: usize,
                                unowned_created: usize,
                                unowned_dangling: usize,
                                cycle_reports_run: usize,
                            }

                            // Ensure per-arena promotion map has entries for all arenas seen (default 0)
                            let mut finalizer_promotions_per_arena = interpreter.finalizer_promotions_per_arena.clone();
                            for aid in interpreter.arena_alloc_count.keys() {
                                finalizer_promotions_per_arena.entry(*aid).or_insert(0usize);
                            }

                            let metrics = Metrics {
                                handled_errors: interpreter.handled_errors,
                                executed_statements: interpreter.executed_statements,
                                crash_free: 100.0
                                    * (1.0
                                        - (interpreter.handled_errors as f64
                                            / interpreter.executed_statements.max(1) as f64)),
                                finalizer_promotions: interpreter.get_finalizer_promotions(),
                                objects_finalized_per_arena: interpreter.objects_finalized_per_arena.clone(),
                                arena_alloc_count: interpreter.arena_alloc_count.clone(),
                                finalizer_promotions_per_arena: finalizer_promotions_per_arena,
                                weak_created: interpreter.weak_created,
                                weak_upgrades: interpreter.weak_upgrades,
                                weak_dangling: interpreter.weak_dangling,
                                unowned_created: interpreter.unowned_created,
                                unowned_dangling: interpreter.unowned_dangling,
                                cycle_reports_run: interpreter.cycle_reports_run.get(),
                            };

                            match serde_json::to_string(&metrics) {
                                Ok(s) => println!("{}", s),
                                Err(e) => {
                                    eprintln!("Failed to serialize metrics: {}", e);
                                    process::exit(70);
                                }
                            }
                        } else {
                            println!("[metrics] handled_errors={} executed_statements={} crash_free={:.1}% finalizer_promotions={}",
                                interpreter.handled_errors,
                                interpreter.executed_statements,
                                100.0 * (1.0 - (interpreter.handled_errors as f64 / interpreter.executed_statements.max(1) as f64)),
                                interpreter.get_finalizer_promotions()
                            );
                            // print a compact arena summary
                            if !interpreter.arena_alloc_count.is_empty() {
                                let arenas: Vec<String> = interpreter.arena_alloc_count.iter().map(|(aid,c)| format!("arena{}:{}alloc", aid, c)).collect();
                                println!("[arena] {}", arenas.join(","));
                            }
                            if !interpreter.objects_finalized_per_arena.is_empty() {
                                let fin: Vec<String> = interpreter.objects_finalized_per_arena.iter().map(|(aid,c)| format!("arena{}:{}finalized", aid, c)).collect();
                                println!("[arena_finalized] {}", fin.join(","));
                            }
                            println!("[mem] weak_created={} weak_upgrades={} weak_dangling={} unowned_created={} unowned_dangling={} cycle_reports_run={}",
                                interpreter.weak_created, interpreter.weak_upgrades, interpreter.weak_dangling,
                                interpreter.unowned_created, interpreter.unowned_dangling, interpreter.cycle_reports_run.get());
                        }
                    }
                    Err(diags) => {
                        for (src, d) in diags {
                            eprintln!("{}", format_diagnostic(&src, &d));
                        }
                        return;
                    }
                }
                // metrics already printed above while `interpreter` was in scope
            }
            Err(e) => {
                eprintln!("Error reading file: {}", e);
                process::exit(74);
            }
        }
        return;
    }
    if args[1] == "add" && args.len() == 3 {
        let src = &args[2];
        // default cache dir
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let cache_dir = std::path::PathBuf::from(format!("{}/.artcode/cache", home));
        let _ = std::fs::create_dir_all(&cache_dir);

        // Determine source kind: local path, file://, or git URL
        let mut working_src: std::path::PathBuf;
        let mut tmp_clone: Option<std::path::PathBuf> = None;

        let src_path = std::path::Path::new(src);
        if src_path.exists() {
            working_src = src_path.to_path_buf();
        } else if let Some(stripped) = src.strip_prefix("file://") {
            let p = std::path::Path::new(stripped);
            if p.exists() {
                working_src = p.to_path_buf();
            } else {
                eprintln!("Source path does not exist: {}", src);
                process::exit(64);
            }
        } else if src.starts_with("git@") || src.starts_with("http://") || src.starts_with("https://") || src.ends_with(".git") {
            // clone into temp dir using system git
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let tmp = std::env::temp_dir().join(format!("art_add_{}", now));
            if let Err(e) = std::fs::create_dir_all(&tmp) {
                eprintln!("Failed to create temp dir for git clone: {}", e);
                process::exit(70);
            }
            let status = std::process::Command::new("git")
                .arg("clone")
                .arg("--depth")
                .arg("1")
                .arg(src)
                .arg(&tmp)
                .status();
            match status {
                Ok(s) if s.success() => {
                    working_src = tmp.clone();
                    tmp_clone = Some(tmp);
                }
                Ok(s) => {
                    eprintln!("git clone failed with status: {}", s);
                    process::exit(70);
                }
                Err(e) => {
                    eprintln!("Failed to spawn git: {}", e);
                    process::exit(70);
                }
            }
        } else {
            eprintln!("Source path does not exist or not a supported URL: {}", src);
            process::exit(64);
        }

        // parse Art.toml if present and require name/version when available
    let mut dest_name = working_src.file_name().and_then(|s| s.to_str()).unwrap_or("pkg").to_string();
    let mut dest_version = "0.0.0".to_string();
    let mut _main_field: Option<String> = None;
        let art_toml = working_src.join("Art.toml");
        if art_toml.exists() {
            if let Ok(s) = std::fs::read_to_string(&art_toml) {
                if let Ok(v) = toml::from_str::<TomlValue>(&s) {
                    if let Some(name_v) = v.get("name").and_then(|n| n.as_str()) {
                        dest_name = name_v.to_string();
                    }
                    if let Some(ver_v) = v.get("version").and_then(|n| n.as_str()) {
                        dest_version = ver_v.to_string();
                    }
                        if let Some(main_v) = v.get("main").and_then(|m| m.as_str()) {
                            _main_field = Some(main_v.to_string());
                        }
                }
            }
        }
        let dest = cache_dir.join(format!("{}-{}", dest_name, dest_version));
        if dest.exists() {
            eprintln!("Destination already exists: {}", dest.display());
            // cleanup clone if we made one
            if let Some(t) = tmp_clone {
                let _ = std::fs::remove_dir_all(t);
            }
            process::exit(65);
        }

        // detect git commit (if any) before copying/cloning cleanup
        let mut commit: Option<String> = None;
        if let Some(tmp) = &tmp_clone {
            // we cloned into tmp; get commit from tmp
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(tmp)
                .arg("rev-parse")
                .arg("HEAD")
                .output();
            if let Ok(o) = out {
                if o.status.success() {
                    if let Ok(s) = String::from_utf8(o.stdout) {
                        commit = Some(s.trim().to_string());
                    }
                }
            }
        } else {
            // if source is a local git repo, try to get commit
            if src_path.exists() {
                let gitdir = src_path.join(".git");
                if gitdir.exists() {
                    let out = std::process::Command::new("git")
                        .arg("-C")
                        .arg(src_path)
                        .arg("rev-parse")
                        .arg("HEAD")
                        .output();
                    if let Ok(o) = out {
                        if o.status.success() {
                            if let Ok(s) = String::from_utf8(o.stdout) {
                                commit = Some(s.trim().to_string());
                            }
                        }
                    }
                }
            }
        }

        // copy file or directory from working_src to dest
        let res = if working_src.is_file() {
            if let Err(e) = std::fs::create_dir_all(&dest) {
                Err(e)
            } else {
                let filename = working_src.file_name().unwrap();
                std::fs::copy(&working_src, dest.join(filename))
            }
        } else {
            // simple directory copy: walk entries
            fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
                std::fs::create_dir_all(to)?;
                for entry in std::fs::read_dir(from)? {
                    let entry = entry?;
                    let ty = entry.file_type()?;
                    let dest = to.join(entry.file_name());
                    if ty.is_dir() {
                        copy_dir(&entry.path(), &dest)?;
                    } else {
                        std::fs::copy(entry.path(), dest)?;
                    }
                }
                Ok(())
            }
            match copy_dir(&working_src, &dest) {
                Ok(()) => Ok(0u64),
                Err(e) => Err(e),
            }
        };

        // cleanup clone if any (after copy)
        if let Some(t) = &tmp_clone {
            let _ = std::fs::remove_dir_all(t);
        }

        match res {
            Ok(_) => {
                println!("Installed to {}", dest.display());

                // write enhanced .art-lock in current directory
                #[derive(Serialize)]
                struct LockEntry {
                    name: String,
                    version: String,
                    source: String,
                    path: String,
                    commit: Option<String>,
                }
                let lock = LockEntry {
                    name: dest_name.clone(),
                    version: dest_version.clone(),
                    source: src.to_string(),
                    path: dest.to_string_lossy().to_string(),
                    commit,
                };
                let _ = std::fs::write(".art-lock", serde_json::to_string_pretty(&lock).unwrap());
                return;
            }
            Err(e) => {
                eprintln!("Failed to add package: {}", e);
                process::exit(70);
            }
        }
    }
    if args[1] == "detect-cycles" {
        let mut json = false;
        let mut json_pretty = false;
        let mut file: Option<&str> = None;
        for a in &args[2..] {
            match a.as_str() {
                "--json" => json = true,
                "--json-pretty" => json_pretty = true,
                _ => file = Some(a),
            }
        }
        if json_pretty {
            json = true;
        }
        let Some(f) = file else {
            println!("Usage: art detect-cycles [--json|--json-pretty] <script>");
            process::exit(64);
        };
        match fs::read_to_string(f) {
            Ok(source) => {
                let mut lexer = Lexer::new(source.clone());
                let tokens = match lexer.scan_tokens() {
                    Ok(t) => t,
                    Err(d) => {
                        eprintln!("{}", format_diagnostic(&source, &d));
                        return;
                    }
                };
                let mut parser = Parser::new(tokens);
                let (program, diags) = parser.parse();
                if !diags.is_empty() {
                    for d in &diags {
                        eprintln!("{}", format_diagnostic(&source, d));
                    }
                    return;
                }
                let mut interp = Interpreter::with_prelude();
                if let Err(e) = interp.interpret(program) {
                    eprintln!("Erro de execução: {}", e);
                }
                if json {
                    println!(
                        "{}",
                        if json_pretty {
                            interp.detect_cycles_json_pretty()
                        } else {
                            interp.detect_cycles_json()
                        }
                    );
                } else {
                    let summary = interp.cycle_report();
                    let det = interp.detect_cycles();
                    println!("cycle_summary: weak_total={} weak_alive={} weak_dead={} unowned_total={} unowned_dangling={} cycle_leaks_detected={}", summary.weak_total, summary.weak_alive, summary.weak_dead, summary.unowned_total, summary.unowned_dangling, interp.cycle_leaks_detected);
                    if !det.cycles.is_empty() {
                        println!("cycles:");
                        for info in det.cycles {
                            println!("  - nodes={:?} isolated={} reachable_root={} leak_candidate={} suggestions={:?} ranked={:?}", info.nodes, info.isolated, info.reachable_from_root, info.leak_candidate, info.suggested_break_edges, info.ranked_suggestions);
                        }
                    } else {
                        println!("cycles: none");
                    }
                    if !det.weak_dead.is_empty() {
                        println!("weak_dead_ids: {:?}", det.weak_dead);
                    }
                    if !det.unowned_dangling.is_empty() {
                        println!("unowned_dangling_ids: {:?}", det.unowned_dangling);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading file: {}", e);
                process::exit(74);
            }
        }
        return;
    }
    println!("Usage: art [run|detect-cycles] [--json] <script>");
    process::exit(64);
}
