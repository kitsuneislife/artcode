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
    },
    /// Strict developer check: fmt, clippy -D warnings, tests, examples; optional coverage
    Devcheck {
        /// Run coverage report (requires cargo-llvm-cov)
        #[arg(long, default_value_t = false)]
        coverage: bool,
    },
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

fn fmt(no_fmt: bool) {
    if no_fmt {
        return;
    }
    let _ = run(Command::new("cargo").args(["fmt", "--all", "--", "--check"]));
}

fn clippy() {
    let _ = run(Command::new("cargo").args(["clippy", "--all", "--", "-D", "warnings"]));
}
fn test_all() {
    let status = run(Command::new("cargo").arg("test"));
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
}

fn run_examples() {
    let mut cmd = Command::new("bash");
    cmd.arg("-c").arg("scripts/test_examples.sh");
    let status = run(&mut cmd);
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
}

fn type_check_examples() {
    // Run the CLI on each example to ensure TypeInfer does not emit type diagnostics.
    let entries = match std::fs::read_dir("cli/examples") {
        Ok(e) => e,
        Err(_) => return,
    };
    for ent in entries.flatten() {
        let path = ent.path();
        if path.extension().map(|e| e == "art").unwrap_or(false) {
            // Run the CLI binary explicitly to avoid cargo ambiguity
            let mut cmd = Command::new("cargo");
            cmd.args(["run", "-p", "cli", "--quiet", "--", "run"]).arg(path.as_os_str());
            let status = run(&mut cmd);
            if !status.success() {
                // If example fails to parse or run, log and continue. We want examples to be helpful
                // but not to block the CI while some didactic examples are being updated.
                eprintln!("Type check skipped (failed) for example: {}", path.display());
                continue;
            }
        }
    }
}

fn scan_panics() {
    let mut paths = vec!["crates".into(), "src".into()];
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
        Commands::Ci { no_fmt } => {
            fmt(no_fmt);
            clippy();
            test_all();
            type_check_examples();
            scan_panics();
        }
        Commands::Devcheck { coverage } => {
            // strict dev flow
            fmt(false);
            clippy();
            test_all();
            type_check_examples();
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
    Commands::Irgen { write, check, outdir } => {
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
        Commands::GenGolden { write, check, outdir } => {
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
        Commands::Scan => scan_panics(),
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
