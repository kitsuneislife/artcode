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
    /// Only scan for potential panics (panic!/unwrap/expect)
    Scan,
    /// Run coverage via cargo-llvm-cov (if installed)
    Coverage {
        #[arg(long, default_value_t = false)]
        html: bool,
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
    let _ = run(Command::new("cargo").args(["clippy", "--all"]));
}
fn test_all() {
    let status = run(Command::new("cargo").arg("test"));
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
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
            scan_panics();
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
