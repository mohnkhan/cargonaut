# Data Model: Cargonaut

**Status**: Phase 1 entities locked. Phases 2-6 add adapters/plugins/audit/undo entities (sketched below).

## Phase 1 entities

### `VfsPath`
```rust
pub struct VfsPath {
    scheme: SmolStr,                   // "file", "sftp", "s3", ...
    authority: Option<SmolStr>,        // "user@host:22" for sftp; None for file
    segments: SmallVec<[SmolStr; 8]>,  // path components, never including '/'
}
impl VfsPath {
    pub fn parse(s: &str) -> Result<Self, ParseError>;
    pub fn display(&self) -> String;        // round-trips through parse()
    pub fn parent(&self) -> Option<Self>;
    pub fn join(&self, segment: &str) -> Self;
}
```
Invariants: `segments` never contains `/` or `..`; `parent()` returns `None` at root; round-trip property tested.

### `VfsMetadata`
```rust
pub struct VfsMetadata {
    pub size: u64,
    pub mtime: SystemTime,
    pub mode: FileMode,                // Unix perms; None on non-Unix
    pub kind: VfsKind,                 // File | Dir | Symlink { target: VfsPath }
    pub is_hidden: bool,
}
```

### `DirListing`
```rust
pub struct DirListing {
    pub entries: Vec<DirEntry>,
    pub sort: Sort,
}
pub struct DirEntry {
    pub name: SmolStr,
    pub meta: VfsMetadata,
}
```
Invariant: entries sorted per `sort` at construction time; UI does not re-sort.

### `TransferJob`
```rust
pub struct TransferJob {
    pub id: TransferId,                // uuid v4
    pub src: VfsRef,                   // (Arc<dyn VfsBackend>, VfsPath)
    pub dst: VfsRef,
    pub mode: TransferMode,            // Copy | Move
    pub state: watch::Receiver<TransferState>,
    pub cancel: CancellationToken,
}

pub enum TransferState {
    Queued,
    Running { bytes_done: u64, bytes_total: u64, eta_secs: u32, throughput_mbps: f32 },
    Paused,
    Completed { sha256_match: bool },
    Failed { error: TransferError, resumable: bool },
    Canceled,
}
```

### `TransferCheckpoint`
```rust
#[derive(Serialize, Deserialize)]
pub struct TransferCheckpoint {
    pub version: u32,                  // schema version; bump on incompatible change
    pub job_id: TransferId,
    pub src_uri: String,
    pub src_size: u64,
    pub src_sha256_prefix: [u8; 32],   // SHA-256 of first 1 MiB; detects source swap
    pub dst_uri: String,
    pub bytes_written: u64,
    pub chunk_crcs: Vec<u32>,          // CRC32 per checkpoint interval; verify on resume
    pub created_at: u64,               // epoch seconds
    pub last_update_at: u64,
}
```
Invariant: `chunk_crcs.len() == bytes_written / chunk_size`. Checkpoint file is fsync'd after every write.

### `Config`
```rust
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Config {
    pub ui: UiConfig,
    pub transfer: TransferConfig,
    pub plugins: PluginsConfig,
    pub credentials: CredentialsConfig,
    pub audit: AuditConfig,
}
```
Full schema in `contracts/config.schema.json`. Round-trip tested via `serde_json::from_str(serde_json::to_string(&Config::default())) == Config::default()`.

## Phase 2-6 entities (sketched)

### `PluginInstance` (Phase 3)
```rust
pub struct PluginInstance {
    pub name: String,
    pub source: PluginSource,          // Wasm { path } | NativeStub { ... }
    pub caps: CapabilitySet,
    pub events: mpsc::Receiver<PluginEvent>,
    pub instance: wasmtime::component::Instance,
}
pub struct CapabilitySet {
    pub read_dir: Vec<VfsPath>,        // allowlist
    pub read_file: bool,               // read any file in any allowed read_dir
    pub write_file: bool,
    pub network: bool,
}
```

### `AuditEntry` (Phase 4)
```rust
pub struct AuditEntry {
    pub ts: SystemTime,
    pub op: AuditOp,                   // Copy | Move | Delete | PluginExec | ...
    pub src: Option<VfsPath>,
    pub dst: Option<VfsPath>,
    pub bytes: u64,
    pub status: AuditStatus,           // Ok | Failed(string) | Canceled
    pub user: String,
    pub hmac: [u8; 32],                // HMAC over (prev_hmac || line_fields)
}
```

### `UndoEntry` (Phase 4)
```rust
pub struct UndoEntry {
    pub op: UndoOp,                    // RestoreFromTrash | ReverseRename | ReverseCopy
    pub data: UndoData,                // op-specific reverse-plan
    pub original_ts: SystemTime,
    pub expires_at: SystemTime,        // GC bound
}
```

## Storage layout

| Path | Purpose | Phase |
|---|---|---|
| `~/.config/cargonaut/config.toml` | user config | 1 |
| `~/.config/cargonaut/themes/*.toml` | theme files | 5 |
| `~/.config/cargonaut/keymap.toml` | optional keymap override | 5 |
| `~/.local/share/cargonaut/audit.log` | audit log (rotated daily → `audit.YYYY-MM-DD.log.gz`) | 4 |
| `~/.local/share/cargonaut/undo/<session>/...` | per-session undo state | 4 |
| `~/.local/share/cargonaut/checkpoints/` (legacy) | NOT used — checkpoints live at dst dir per R3 | — |
| `~/.local/share/cargonaut/plugins/` | installed WASM modules | 3 |
| `~/.cache/cargonaut/preview-cache/` | rendered preview tiles | 3 |
| `~/.cache/cargonaut/locale-cache/` | compiled fluent bundles | 5 |
