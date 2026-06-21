# Contract — `cargonaut-core::diag` public API

Stable surface other crates depend on. All items carry `///` docs
(`#![warn(missing_docs)]`). Signatures indicative; names are the contract.

## Recent-action trail

```rust
/// Record one user action/command into the process-global ring buffer.
/// `detail` MUST be secret-free (no paths' contents, no credentials).
pub fn record_action(label: &str, detail: Option<&str>);

/// Snapshot the trail, oldest first. Cheap clone of ≤ 64 entries.
pub fn recent_actions() -> Vec<ActionRecord>;

pub struct ActionRecord { pub seq: u64, pub label: String, pub detail: Option<String> }
```

- Capacity 64, oldest dropped. Thread-safe. `record_action` is O(1) and lock-cheap.

## Panic capture

```rust
/// Install the global panic hook (idempotent). Captures message/location/
/// thread/backtrace + an action snapshot into a global slot and logs at error.
/// Does NOT touch the terminal or write files.
pub fn install_panic_hook();

/// Take the most recently captured panic, if any (consumed by a catch site).
pub fn take_captured_panic() -> Option<CapturedPanic>;

pub struct CapturedPanic {
    pub message: String,
    pub location: Option<String>,
    pub thread: String,
    pub backtrace: String,
    pub actions: Vec<ActionRecord>,
}
```

- `install_panic_hook` chains to no prior hook output (suppresses the default
  stderr dump, which would corrupt the alternate screen).

## Crash report (pure formatter)

```rust
pub struct ReportMeta { pub app_version: &'static str, pub os: &'static str,
                        pub arch: &'static str, pub timestamp: String }

/// Deterministic, IO-free. Output == on-disk crash file content.
/// Guaranteed to contain version, platform, location (if any), and backtrace.
/// Guaranteed to contain no credentials (built only from secret-free inputs).
pub fn format_crash_report(meta: &ReportMeta, panic: &CapturedPanic) -> String;
```

## Crash log (IO; dir-injectable; never panics)

```rust
pub fn write_report(dir: &Path, timestamp: &str, body: &str) -> io::Result<PathBuf>;
pub fn prune_reports(dir: &Path, keep: usize) -> io::Result<()>;          // default keep = 10
pub fn unseen_report(dir: &Path) -> io::Result<Option<PathBuf>>;
pub fn mark_seen(dir: &Path, report: &Path) -> io::Result<()>;

/// Resolve the data dir ($XDG_DATA_HOME/cargonaut or ~/.local/share/cargonaut).
pub fn data_dir() -> PathBuf;
```

- Every function returns `io::Result`; callers degrade gracefully on `Err`
  (FR-013). No function may `unwrap`/`panic` on IO error.

## About

```rust
pub struct AboutInfo { /* name, version, author, copyright, license, repository */ }
pub fn about() -> AboutInfo;
pub fn about_lines() -> Vec<String>;   // shared by help, dialog, clap long_version
```

## Test injection

```rust
/// If `CARGONAUT_PANIC_INJECT` equals `site`, panic once. Inert when unset.
pub fn maybe_inject_panic(site: &str);   // sites: "startup" | "render" | "input" | "task"
```
