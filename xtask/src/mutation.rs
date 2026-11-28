//! `cargo xtask mutation`: runs cargo-mutants and prints Tau's mutation report on stdout.
//!
//! Exit codes follow Tau's contract: 0 for `pass`, `nothing`, or survivors in audit mode, 2 for
//! survivors in changed mode, and 1 for an error or any unmutated file. Everything except the
//! report goes to stderr, because Tau parses stdout as JSON.

use std::collections::BTreeMap;
use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use anyhow::Context as _;
use serde_json::Value;
use thiserror::Error;

mod report;
mod request;
mod survivors;

use report::{ran_report, report_value};
use request::{Mode, Request, Shard, Target, parse_request};
use survivors::{Survivor, read_survivors};

struct Scope {
    diff: Option<PathBuf>,
    files: Vec<String>,
}

enum Outcome {
    Nothing,
    Ran {
        survivors: Vec<Survivor>,
        unmutated: Vec<String>,
    },
}

#[derive(Debug, Error)]
enum PathError {
    #[error("Could not find {path}")]
    Missing {
        path: String,
        source: std::io::Error,
    },
    #[error("Could not resolve {root}")]
    UnresolvedRoot {
        root: PathBuf,
        source: std::io::Error,
    },
    #[error("{path} is outside the workspace.")]
    Outside { path: String },
}

#[derive(Debug, PartialEq, Eq, Error)]
enum MemoryError {
    #[error("{MEMORY_INFO} has no MemTotal line.")]
    MissingTotal,
    #[error("MemTotal in {MEMORY_INFO} is not in kB.")]
    NotKilobytes,
    #[error("MemTotal in {MEMORY_INFO} is not a number.")]
    NotANumber,
}

const OUTPUT_DIRECTORY: &str = "target/mutants";
// cargo-mutants builds each job's copy of the workspace under TMPDIR. `/tmp` is often a tmpfs held
// in memory, so the copies go to this folder on disk instead.
const SCRATCH_DIRECTORY: &str = "target/mutants/scratch";
const MAX_JOBS: usize = 4;
const MEMORY_INFO: &str = "/proc/meminfo";
const KILOBYTE: u64 = 1024;
// Half of the machine's memory is left for cargo, rustc, and the rest of the machine, so several
// jobs cannot fill it with capped test processes.
const MEMORY_HEADROOM: u64 = 2;
const MUTATION_PROFILE: &str = "mutation";
const TEST_MEMORY_VARIABLE: &str = "MUTATION_TEST_MEMORY_BYTES";

impl Scope {
    fn arguments(&self) -> Vec<String> {
        let mut arguments = Vec::new();

        if let Some(diff) = &self.diff {
            arguments.push("--in-diff".to_string());
            arguments.push(diff.display().to_string());
        }

        for file in &self.files {
            arguments.push("--file".to_string());
            arguments.push(file.clone());
        }

        arguments
    }
}

fn workspace_relative_path(root: &Path, path: &str) -> Result<String, PathError> {
    let file = root
        .join(path)
        .canonicalize()
        .map_err(|source| PathError::Missing {
            path: path.to_string(),
            source,
        })?;

    let canonical_root = root
        .canonicalize()
        .map_err(|source| PathError::UnresolvedRoot {
            root: root.to_path_buf(),
            source,
        })?;

    let relative = file
        .strip_prefix(&canonical_root)
        .map_err(|_| PathError::Outside {
            path: path.to_string(),
        })?;

    let parts: Vec<String> = relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();

    Ok(parts.join("/"))
}

fn write_diff(
    git: &str,
    root: &Path,
    output: &Path,
    base: &str,
) -> anyhow::Result<Option<PathBuf>> {
    let diff = Command::new(git)
        .args(["diff", "--no-ext-diff", base, "--"])
        .current_dir(root)
        .stderr(Stdio::inherit())
        .output()
        .context("Could not start git diff")?;

    if !diff.status.success() {
        anyhow::bail!("git diff {base} failed with {}.", diff.status);
    }

    if diff.stdout.is_empty() {
        return Ok(None);
    }

    let path = output.join("changes.diff");

    fs::write(&path, &diff.stdout)
        .with_context(|| format!("Could not write {}", path.display()))?;

    Ok(Some(path))
}

fn count_mutants(
    cargo: &str,
    root: &Path,
    scope: &Scope,
) -> anyhow::Result<BTreeMap<String, usize>> {
    let listing = Command::new(cargo)
        .args(["mutants", "--workspace", "--list", "--json"])
        .args(scope.arguments())
        .current_dir(root)
        .stderr(Stdio::inherit())
        .output()
        .context("Could not start cargo mutants")?;

    if !listing.status.success() {
        anyhow::bail!("cargo mutants --list failed with {}.", listing.status);
    }

    if listing.stdout.trim_ascii().is_empty() {
        return Ok(BTreeMap::new());
    }

    let mutants: Value = serde_json::from_slice(&listing.stdout)
        .with_context(|| "cargo mutants --list printed invalid JSON")?;

    let mutants = mutants
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("cargo mutants --list did not print a JSON array."))?;

    let mut counts = BTreeMap::new();

    for mutant in mutants {
        let file = mutant["file"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("A listed mutant has no file."))?;

        *counts.entry(file.to_string()).or_insert(0) += 1;
    }

    Ok(counts)
}

fn apply_cap(
    counts: &BTreeMap<String, usize>,
    max_mutants: Option<usize>,
) -> (Vec<String>, Vec<String>) {
    let mut remaining = max_mutants.unwrap_or(usize::MAX);
    let mut selected = Vec::new();
    let mut unmutated = Vec::new();

    for (file, &count) in counts {
        if count <= remaining {
            remaining -= count;
            selected.push(file.clone());
        } else {
            unmutated.push(file.clone());
        }
    }

    (selected, unmutated)
}

fn test_memory_limit(total: u64, jobs: usize) -> u64 {
    total / MEMORY_HEADROOM / jobs as u64
}

fn parse_total_memory(contents: &str) -> Result<u64, MemoryError> {
    let total = contents
        .lines()
        .find_map(|line| line.strip_prefix("MemTotal:"))
        .ok_or(MemoryError::MissingTotal)?;

    let kilobytes = total
        .trim()
        .strip_suffix("kB")
        .ok_or(MemoryError::NotKilobytes)?;

    let kilobytes: u64 = kilobytes
        .trim()
        .parse()
        .map_err(|_| MemoryError::NotANumber)?;

    Ok(kilobytes * KILOBYTE)
}

fn total_memory() -> anyhow::Result<u64> {
    let read_error = match fs::read_to_string(MEMORY_INFO) {
        Ok(contents) => return Ok(parse_total_memory(&contents)?),
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            return Err(error).with_context(|| format!("Could not read {MEMORY_INFO}"));
        }
        Err(error) => error,
    };

    // macOS keeps no /proc, so the kernel reports the memory size instead.
    let output = Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .with_context(|| format!("Could not read {MEMORY_INFO} ({read_error}) or sysctl"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);

        anyhow::bail!(
            "Could not read {MEMORY_INFO} ({read_error}) and sysctl hw.memsize failed with {}: {}",
            output.status,
            stderr.trim()
        );
    }

    parse_memory_size(&output.stdout)
}

fn parse_memory_size(bytes: &[u8]) -> anyhow::Result<u64> {
    let text = std::str::from_utf8(bytes).context("hw.memsize is not UTF-8")?;

    text.trim().parse().context("hw.memsize is not a number")
}

fn available_cores() -> NonZeroUsize {
    std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN)
}

fn job_counts(cores: NonZeroUsize) -> (usize, usize) {
    let jobs = cores.get().min(MAX_JOBS);
    let threads = cores.get() / jobs;

    (jobs, threads)
}

fn scratch_directory(root: &Path) -> PathBuf {
    root.join(SCRATCH_DIRECTORY)
}

fn empty_scratch(root: &Path) -> anyhow::Result<()> {
    let scratch = scratch_directory(root);

    if scratch.exists() {
        fs::remove_dir_all(&scratch)
            .with_context(|| format!("Could not empty {}", scratch.display()))?;
    }

    fs::create_dir_all(&scratch)
        .with_context(|| format!("Could not create {}", scratch.display()))?;

    Ok(())
}

struct Mutants<'a> {
    cargo: &'a str,
    root: &'a Path,
    output: &'a Path,
    shard: Option<Shard>,
}

impl Mutants<'_> {
    fn command(&self, scope: &Scope, process_memory_limit: u64) -> Command {
        let (jobs, threads) = job_counts(available_cores());
        // Each job's nextest run defaults to one test process per core, so the jobs together would
        // start jobs times cores test processes. Cap the test threads so the jobs together keep to
        // the core count.
        let mut command = Command::new(self.cargo);

        command
            .env("TMPDIR", scratch_directory(self.root))
            .env("NEXTEST_TEST_THREADS", threads.to_string())
            .env("NEXTEST_PROFILE", MUTATION_PROFILE)
            .env(TEST_MEMORY_VARIABLE, process_memory_limit.to_string())
            .args(["mutants", "--workspace", "--no-shuffle", "--jobs"])
            .arg(jobs.to_string())
            .arg("--output")
            .arg(self.output);

        if let Some(shard) = self.shard {
            let argument = format!("{}/{}", shard.index, shard.count);

            // Round-robin spreads each file's mutants over the shards, so one slow file does not
            // lengthen a single shard.
            command.args(["--shard", &argument, "--sharding", "round-robin"]);
        }

        command
            .args(scope.arguments())
            .current_dir(self.root)
            .stdout(std::io::stderr());

        command
    }

    fn run(&self, scope: &Scope) -> anyhow::Result<()> {
        empty_scratch(self.root)?;
        let (jobs, _) = job_counts(available_cores());
        let limit = test_memory_limit(total_memory()?, jobs);
        let status = self
            .command(scope, limit)
            .status()
            .context("Could not start cargo mutants")?;

        match status.code() {
            Some(0 | 2 | 3) => Ok(()),
            Some(4) => anyhow::bail!("The unmutated tree fails to build or pass its tests."),
            _ => anyhow::bail!("cargo mutants failed with {status}."),
        }
    }
}

fn mutate(root: &Path, request: &Request, cargo: &str, git: &str) -> anyhow::Result<Outcome> {
    let output = root.join(OUTPUT_DIRECTORY);

    fs::create_dir_all(&output)
        .with_context(|| format!("Could not create {}", output.display()))?;

    let scope = match &request.target {
        Target::Changed { base } => {
            let Some(diff) = write_diff(git, root, &output, base)? else {
                return Ok(Outcome::Nothing);
            };

            Scope {
                diff: Some(diff),
                files: Vec::new(),
            }
        }
        Target::Audit { files } => Scope {
            diff: None,
            files: files
                .iter()
                .map(|path| workspace_relative_path(root, path))
                .collect::<Result<Vec<_>, PathError>>()?,
        },
    };

    let counts = count_mutants(cargo, root, &scope)?;

    if counts.is_empty() {
        return Ok(Outcome::Nothing);
    }

    let (selected, unmutated) = apply_cap(&counts, request.max_mutants);

    if selected.is_empty() {
        return Ok(Outcome::Ran {
            survivors: Vec::new(),
            unmutated,
        });
    }

    let mutants = Mutants {
        cargo,
        root,
        output: &output,
        shard: request.shard,
    };
    let selected_scope = Scope {
        files: selected,
        ..scope
    };

    mutants.run(&selected_scope)?;

    let survivors = read_survivors(root, &output)?;

    Ok(Outcome::Ran {
        survivors,
        unmutated,
    })
}

fn run_request(
    root: &Path,
    arguments: &[String],
    cargo: &str,
    git: &str,
) -> anyhow::Result<(Mode, Outcome)> {
    let request = parse_request(arguments)?;

    let outcome = mutate(root, &request, cargo, git)?;

    Ok((request.target.mode(), outcome))
}

fn report(root: &Path, arguments: &[String], cargo: &str, git: &str) -> (Value, u8) {
    match run_request(root, arguments, cargo, git) {
        Ok((_, Outcome::Nothing)) => (report_value("nothing", None, &[], &[]), 0),
        Ok((
            mode,
            Outcome::Ran {
                survivors,
                unmutated,
            },
        )) => ran_report(mode, &survivors, &unmutated),
        Err(error) => (
            report_value("error", Some(&format!("{error:#}")), &[], &[]),
            1,
        ),
    }
}

#[expect(
    clippy::print_stdout,
    reason = "the mutation report is the command's stdout output"
)]
pub(crate) fn run(root: &Path, arguments: &[String]) -> ExitCode {
    let (report, exit) = report(root, arguments, "cargo", "git");

    println!("{report}");
    ExitCode::from(exit)
}

#[cfg(test)]
mod tests;
