use clap::{Parser, Subcommand};
use regex::Regex;
use std::path::PathBuf;
use std::process::{Command, ExitStatus};

#[derive(Parser)]
#[command(author, version, about="Developer tasks for Artcode", long_about=None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run full developer quality gate (fmt, clippy, test, panic scan)
    Ci {
        #[arg(long)]
        no_fmt: bool,
        /// When set, fail CI if aot_inspect finds issues
        #[arg(long, default_value_t = false)]
        aot_inspect_fatal: bool,
    },
    /// Strict developer check: fmt, clippy -D warnings, tests, examples; optional coverage
    Devcheck {
        /// Run coverage report (requires cargo-llvm-cov)
        #[arg(long, default_value_t = false)]
        coverage: bool,
    },
    /// Run every example under `examples/` and fail on any regression
    RunExamples,
    /// Only scan for potential panics (panic!/unwrap/expect)
    Scan,
    /// Run coverage via cargo-llvm-cov (if installed)
    Coverage {
        #[arg(long, default_value_t = false)]
        html: bool,
    },
    /// Generate or verify IR golden files
    Irgen {
        /// write golden files instead of printing
        #[arg(long)]
        write: bool,
        /// check existing golden files against generated output
        #[arg(long)]
        check: bool,
        /// output directory for golden files (default: crates/ir/golden)
        #[arg(long)]
        outdir: Option<PathBuf>,
    },
    /// Alias for Irgen (gen-golden)
    GenGolden {
        /// write golden files instead of printing
        #[arg(long)]
        write: bool,
        /// check existing golden files against generated output
        #[arg(long)]
        check: bool,
        /// output directory for golden files (default: crates/ir/golden)
        #[arg(long)]
        outdir: Option<PathBuf>,
    },
    /// Emit IR for examples or fixtures (prints textual IR or writes to outdir)
    EmitIr {
        /// optional output directory for IR files
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Inspect AOT plan against profile and optional IR dir
    AotInspect {
        /// profile json path
        #[arg(long)]
        profile: Option<PathBuf>,
        /// aot plan json path
        #[arg(long)]
        plan: Option<PathBuf>,
        /// optional IR directory to estimate cost
        #[arg(long)]
        ir_dir: Option<PathBuf>,
    },
    /// Benchmark cold-start of the `art run` command (50 samples, fails if median > 10ms)
    BenchStartup {
        /// Script to run for each sample (default: examples/00_hello.art)
        #[arg(long, default_value = "examples/00_hello.art")]
        script: String,
        /// Threshold in milliseconds — fails if median exceeds this value (default: 10)
        #[arg(long, default_value_t = 10u64)]
        threshold_ms: u64,
    },
}

fn run(cmd: &mut Command) -> ExitStatus {
    println!("==> {:?}", cmd);
    match cmd.status() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to run command: {}", e);
            std::process::exit(1);
        }
    }
}

/// Runs a command and aborts the whole gate when it fails.
///
/// Every quality check funnels through here so that a failure can never be
/// silently discarded — the previous `let _ = run(..)` on fmt and clippy made
/// `devcheck` unable to fail on the two things CI enforces with `-D warnings`,
/// which is how a clippy regression reached CI while the local gate was green.
fn run_or_exit(cmd: &mut Command) {
    let status = run(cmd);
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
}

fn fmt(no_fmt: bool) {
    if no_fmt {
        return;
    }
    run_or_exit(Command::new("cargo").args(["fmt", "--all", "--", "--check"]));
}

/// Same flags as the `lint` job in `.github/workflows/ci.yml`.
///
/// `--all-targets` matters: without it tests, benches and examples are never
/// linted locally, so a warning in a test file only surfaces in CI.
fn clippy() {
    run_or_exit(Command::new("cargo").args([
        "clippy",
        "--workspace",
        "--all-targets",
        "--locked",
        "--",
        "-D",
        "warnings",
    ]));
}

fn test_all() {
    run_or_exit(Command::new("cargo").args(["test", "--workspace", "--locked"]));
}

/// Collects every `.art` file under `examples/`, recursively, sorted.
///
/// Recursion is deliberate: the previous shell runner globbed
/// `examples/[0-9][0-9]_*.art`, which silently skipped `examples/artkit/` and
/// `examples/modules/`. All of them run under `art run`.
fn collect_examples(dir: &PathBuf, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("cannot read {}: {}", dir.display(), e);
            std::process::exit(1);
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_examples(&path, out);
        } else if path.extension().map(|e| e == "art").unwrap_or(false) {
            out.push(path);
        }
    }
}

/// Executes every example and fails if any of them regresses.
///
/// Ported from `scripts/test_examples.sh` so the gate runs on Windows too,
/// where the project is developed but bash is not guaranteed. Output goes to
/// `target/` instead of `examples/_outputs/` so running the gate never dirties
/// the working tree.
fn run_examples() {
    run_or_exit(Command::new("cargo").args(["build", "--locked", "-p", "cli", "--bin", "art"]));

    let bin = PathBuf::from("target")
        .join("debug")
        .join(format!("art{}", std::env::consts::EXE_SUFFIX));

    let mut examples = Vec::new();
    collect_examples(&PathBuf::from("examples"), &mut examples);
    examples.sort();

    let out_dir = PathBuf::from("target").join("example-output");
    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        eprintln!("cannot create {}: {}", out_dir.display(), e);
        std::process::exit(1);
    }

    let mut failed: Vec<String> = Vec::new();
    for path in &examples {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        println!("[run] {}", path.display());

        let output = match Command::new(&bin).arg("run").arg(path).output() {
            Ok(o) => o,
            Err(e) => {
                eprintln!("failed to spawn {}: {}", bin.display(), e);
                std::process::exit(1);
            }
        };

        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = std::fs::write(out_dir.join(format!("{}.out", name)), &output.stdout);
        let _ = std::fs::write(out_dir.join(format!("{}.err", name)), stderr.as_bytes());

        // A zero exit status is not enough: the interpreter can report a panic
        // on stderr while the process still terminates cleanly.
        let reason = if !output.status.success() {
            Some(format!("exit status {}", output.status))
        } else if stderr.to_lowercase().contains("panic") {
            Some("panic on stderr".to_string())
        } else if stderr.contains("thread '") {
            Some("thread crash on stderr".to_string())
        } else {
            None
        };

        if let Some(reason) = reason {
            eprintln!("[fail] {}: {}", path.display(), reason);
            eprintln!("{}", stderr);
            failed.push(path.display().to_string());
        }
    }

    if failed.is_empty() {
        println!("All {} examples ran successfully.", examples.len());
    } else {
        eprintln!("{} of {} examples failed:", failed.len(), examples.len());
        for f in &failed {
            eprintln!("  {}", f);
        }
        std::process::exit(1);
    }
}

fn scan_panics() {
    let mut paths = vec!["crates".into(), "cli".into(), "xtask".into()];
    let re = match Regex::new(r"panic!|unwrap\(|expect\(") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("invalid panic-scan regex: {}", e);
            std::process::exit(1);
        }
    };
    let mut found = 0usize;
    for p in paths.drain(..) {
        visit(&p, &re, &mut found);
    }
    if found == 0 {
        println!("No potential panics found.");
    } else {
        eprintln!("Found {found} potential panic sites.");
    }
}

fn visit(path: &PathBuf, re: &Regex, found: &mut usize) {
    if path.is_dir() {
        if let Ok(rd) = std::fs::read_dir(path) {
            for entry_res in rd {
                match entry_res {
                    Ok(entry) => visit(&entry.path(), re, found),
                    Err(e) => eprintln!("skipping entry in {:?}: {}", path, e),
                }
            }
        } else {
            eprintln!("cannot read dir {:?}", path);
        }
    } else if let Some(ext) = path.extension() {
        if ext == "rs" {
            if let Ok(txt) = std::fs::read_to_string(path) {
                for (i, line) in txt.lines().enumerate() {
                    if re.is_match(line) {
                        *found += 1;
                        println!("{}:{}:{}", path.display(), i + 1, line.trim());
                    }
                }
            }
        }
    }
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Ci {
            no_fmt,
            aot_inspect_fatal,
        } => {
            fmt(no_fmt);
            clippy();
            test_all();
            run_examples();
            scan_panics();
            // Run AOT inspection; optionally fail the CI when issues are found.
            let mut cmd = Command::new("cargo");
            cmd.args(["run", "-p", "jit", "--bin", "aot_inspect", "--quiet"]);
            cmd.arg("--");
            cmd.arg("profile.json");
            cmd.arg("aot_plan.json");
            let status = run(&mut cmd);
            if !status.success() {
                if aot_inspect_fatal {
                    eprintln!("aot_inspect failed and --aot-inspect-fatal was set; failing CI");
                    std::process::exit(status.code().unwrap_or(1));
                } else {
                    eprintln!("aot_inspect failed (non-fatal)");
                }
            }
        }
        Commands::Devcheck { coverage } => {
            // strict dev flow
            fmt(false);
            clippy();
            test_all();
            run_examples();
            scan_panics();
            if coverage {
                // reuse Coverage branch
                let mut cmd = Command::new("cargo");
                cmd.args([
                    "llvm-cov",
                    "--workspace",
                    "--ignore-filename-regex",
                    ".*/target/.*",
                ]);
                cmd.arg("--html");
                let status = run(&mut cmd);
                if !status.success() {
                    std::process::exit(status.code().unwrap_or(1));
                }
            }
        }
        Commands::Irgen {
            write,
            check,
            outdir,
        } => {
            // Run the irgen binary to print, write, or check golden files
            let mut cmd = Command::new("cargo");
            cmd.args(["run", "-p", "ir", "--bin", "irgen", "--quiet"]);
            if write || check || outdir.is_some() {
                cmd.arg("--");
                if write {
                    cmd.arg("--write");
                }
                if check {
                    cmd.arg("--check");
                }
                if let Some(p) = outdir {
                    cmd.arg("--outdir").arg(p.as_os_str());
                }
            }
            let status = run(&mut cmd);
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }
        Commands::GenGolden {
            write,
            check,
            outdir,
        } => {
            // alias for Irgen
            let mut cmd = Command::new("cargo");
            cmd.args(["run", "-p", "ir", "--bin", "irgen", "--quiet"]);
            if write || check || outdir.is_some() {
                cmd.arg("--");
                if write {
                    cmd.arg("--write");
                }
                if check {
                    cmd.arg("--check");
                }
                if let Some(p) = outdir {
                    cmd.arg("--outdir").arg(p.as_os_str());
                }
            }
            let status = run(&mut cmd);
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }
        Commands::EmitIr { path } => {
            let mut cmd = Command::new("cargo");
            cmd.args(["run", "-p", "ir", "--bin", "irgen", "--quiet"]);
            if let Some(p) = path {
                cmd.arg("--").arg("--outdir").arg(p.as_os_str());
            }
            let status = run(&mut cmd);
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }
        Commands::AotInspect {
            profile,
            plan,
            ir_dir,
        } => {
            let mut cmd = Command::new("cargo");
            cmd.args(["run", "-p", "jit", "--bin", "aot_inspect", "--quiet"]);
            cmd.arg("--");
            if let Some(p) = profile {
                cmd.arg(p.as_os_str());
            } else {
                cmd.arg("profile.json");
            }
            if let Some(p) = plan {
                cmd.arg(p.as_os_str());
            } else {
                cmd.arg("aot_plan.json");
            }
            if let Some(d) = ir_dir {
                cmd.arg(d.as_os_str());
            }
            let status = run(&mut cmd);
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }
        Commands::RunExamples => run_examples(),
        Commands::Scan => scan_panics(),
        Commands::BenchStartup {
            script,
            threshold_ms,
        } => {
            // Ensure we have a release binary to measure
            println!("[bench-startup] Building release binary...");
            let build_status =
                run(Command::new("cargo").args(["build", "--bin", "art", "--release", "--quiet"]));
            if !build_status.success() {
                eprintln!("[bench-startup] Release build failed");
                std::process::exit(1);
            }

            let samples = 50usize;
            let mut times_us: Vec<u128> = Vec::with_capacity(samples);
            println!("[bench-startup] Running '{}' {} times...", script, samples);

            for _ in 0..samples {
                let start = std::time::Instant::now();
                let status = std::process::Command::new("target/release/art")
                    .args(["run", &script])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
                let elapsed = start.elapsed();
                match status {
                    Ok(s) if s.success() => times_us.push(elapsed.as_micros()),
                    _ => {
                        eprintln!("[bench-startup] Sample run failed for {}", script);
                        std::process::exit(1);
                    }
                }
            }

            times_us.sort_unstable();
            let min_ms = *times_us.first().unwrap() as f64 / 1000.0;
            let max_ms = *times_us.last().unwrap() as f64 / 1000.0;
            let median_ms = times_us[samples / 2] as f64 / 1000.0;
            let mean_ms = times_us.iter().sum::<u128>() as f64 / samples as f64 / 1000.0;

            println!("[bench-startup] Results ({} samples):", samples);
            println!("  min   : {:.3}ms", min_ms);
            println!("  median: {:.3}ms", median_ms);
            println!("  mean  : {:.3}ms", mean_ms);
            println!("  max   : {:.3}ms", max_ms);
            println!("  threshold: {}ms", threshold_ms);

            if median_ms > threshold_ms as f64 {
                eprintln!(
                    "[bench-startup] FAIL: median {:.3}ms exceeds threshold {}ms",
                    median_ms, threshold_ms
                );
                std::process::exit(1);
            } else {
                println!(
                    "[bench-startup] OK: median {:.3}ms is within {}ms threshold ✓",
                    median_ms, threshold_ms
                );
            }
        }
        Commands::Coverage { html } => {
            // Detect cargo-llvm-cov
            let tool = Command::new("bash")
                .arg("-c")
                .arg("command -v cargo-llvm-cov")
                .status()
                .ok()
                .filter(|s| s.success())
                .is_some();
            if !tool {
                eprintln!("cargo-llvm-cov not found. Install with: cargo install cargo-llvm-cov",);
                std::process::exit(1);
            }
            let mut cmd = Command::new("cargo");
            cmd.args([
                "llvm-cov",
                "--workspace",
                "--ignore-filename-regex",
                ".*/target/.*",
            ]);
            if html {
                cmd.arg("--html");
            }
            let status = run(&mut cmd);
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }
    }
}
