use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use clap::Parser;
use threadpool::ThreadPool;

#[derive(Parser)]
#[command(name = "harness", about = "CI harness for caelum")]
struct Cli {
    #[arg(short, long, default_value_t = default_jobs())]
    jobs: usize,
}

fn default_jobs() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

enum Outcome {
    Pass,
    Fail(String),
    Timeout,
}

struct JobResult {
    name: String,
    outcome: Outcome,
}

fn run_with_timeout(cmd: &str, args: &[&str], timeout: Duration) -> Outcome {
    let child = match Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return Outcome::Fail(format!("spawn error: {e}")),
    };

    let (tx, rx) = mpsc::channel();
    let id = child.id();

    std::thread::spawn(move || {
        let result = child.wait_with_output();
        let _ = tx.send(result);
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok(output)) => {
            if output.status.success() {
                Outcome::Pass
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                let detail = if !stderr.is_empty() {
                    stderr.trim().to_string()
                } else if !stdout.is_empty() {
                    stdout.trim().to_string()
                } else {
                    format!("exit {}", output.status)
                };
                Outcome::Fail(detail)
            }
        }
        Ok(Err(e)) => Outcome::Fail(format!("wait error: {e}")),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            let _ = Command::new("kill")
                .args(["-9", &id.to_string()])
                .status();
            Outcome::Timeout
        }
        Err(e) => Outcome::Fail(format!("channel error: {e}")),
    }
}

fn collect_lum_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_recursive(dir, &mut files);
    files.sort();
    files
}

fn collect_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_recursive(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("lum") {
            out.push(path);
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let timeout = Duration::from_secs(60);

    println!("harness: building project...");
    let build = Command::new("cargo")
        .args(["build", "--quiet"])
        .status();
    match build {
        Ok(s) if s.success() => {}
        Ok(s) => {
            println!("FAIL  build (exit {})", s.code().unwrap_or(-1));
            return ExitCode::from(1);
        }
        Err(e) => {
            println!("FAIL  build ({e})");
            return ExitCode::from(1);
        }
    }

    let lum_files = collect_lum_files(Path::new("examples"));
    let total_jobs = 1 + lum_files.len(); // 1 for tests
    let pool = ThreadPool::new(cli.jobs);
    let (tx, rx) = mpsc::channel::<JobResult>();

    // Submit test job
    {
        let tx = tx.clone();
        pool.execute(move || {
            let outcome = run_with_timeout("cargo", &["test", "--quiet"], timeout);
            let _ = tx.send(JobResult {
                name: "cargo test".to_string(),
                outcome,
            });
        });
    }

    // Submit example jobs
    for path in lum_files {
        let tx = tx.clone();
        pool.execute(move || {
            let name = path.display().to_string();
            let outcome =
                run_with_timeout("cargo", &["run", "--quiet", "--", &name], timeout);
            let _ = tx.send(JobResult { name, outcome });
        });
    }

    drop(tx);

    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut timed_out = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for result in rx.iter().take(total_jobs) {
        match &result.outcome {
            Outcome::Pass => {
                println!("PASS  {}", result.name);
                passed += 1;
            }
            Outcome::Fail(detail) => {
                println!("FAIL  {}", result.name);
                println!("      {detail}");
                failures.push(result.name.clone());
                failed += 1;
            }
            Outcome::Timeout => {
                println!("TIME  {}", result.name);
                failures.push(format!("{} (timeout)", result.name));
                timed_out += 1;
            }
        }
    }

    println!();
    println!(
        "harness: {passed} passed, {failed} failed, {timed_out} timed out ({total_jobs} total)"
    );

    if !failures.is_empty() {
        println!();
        for f in &failures {
            println!("  - {f}");
        }
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
