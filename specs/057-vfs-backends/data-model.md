# Data Model: VFS Backends — Archives + Remote (SFTP/FTP)

## VfsRegistry

```
VfsRegistry
  local: Arc<dyn VfsBackend>           # always LocalFs, keyed "file"
  remote_map: HashMap<SmolStr, Arc<dyn VfsBackend>>
                                        # key = "{scheme}://{authority}"
                                        # e.g. "sftp://alice@host:22"
```

- **Lifecycle**: Created once in `App::new`; wrapped in `Arc<VfsRegistry>` and held by `App`.
- **Lookup**: `resolve(path)` → check `remote_map["{scheme}://{authority}"]` if authority present; fall back to `local()` for `file://`; return `None` for unregistered schemes.
- **Archive backends are NOT stored here** — they are held directly by `PaneState.backend`.

---

## PaneState (updated)

```
PaneState
  cwd:     VfsPath                     # existing field (unchanged type)
  backend: Arc<dyn VfsBackend>         # NEW — the backend serving this pane's cwd
  listing: DirListing                  # existing
  cursor:  usize                       # existing
  ...                                  # all other existing fields unchanged
```

- **Invariant**: `backend.scheme() == cwd.scheme` at all times. `navigate_to` enforces this.
- **Local panes**: `backend = registry.local()`, `cwd.scheme = "file"`.
- **Archive panes**: `backend = Arc<ZipFs/TarFs>`, `cwd.scheme = "zip"/"tar"`.
- **Remote panes**: `backend = registry.resolve(&cwd).unwrap()`, `cwd.scheme = "sftp"/"ftp"`.

---

## ZipFs (archives feature)

```
ZipFs
  archive_path: PathBuf                # absolute host filesystem path to the .zip file
  index: Arc<Mutex<ZipIndex>>

ZipIndex
  archive: ZipArchive<BufReader<File>>
  entries: Vec<ZipEntryMeta>           # ordered as in ZIP central directory
  name_to_idx: HashMap<String, usize>  # normalized entry path → index into entries

ZipEntryMeta
  path:        String                  # normalized entry path (e.g. "subdir/file.txt")
  size:        u64                     # uncompressed size
  mtime:       SystemTime
  is_dir:      bool
  is_encrypted:bool
  compression: CompressionMethod       # Stored, Deflated, etc.
```

- **VfsPath mapping**: `zip://{percent-encoded-archive-host-path}/{entry/path/segments}`
  - `authority` decoded via `VfsPath::decode_authority()` → `archive_path`
  - `segments` joined with `/` → entry path within archive
  - Root of archive: authority = encoded archive path, segments = []
- **Capabilities**: `VfsCaps::empty()` (no SEEKABLE, no RANDOM_WRITE, etc.)
- **Write ops**: all return `VfsError::Unsupported`

---

## TarFs (archives feature)

```
TarFs
  archive_path: PathBuf
  compression:  TarCompression         # None | Gz | Bz2 | Xz
  index: Arc<TarIndex>

TarCompression = None | Gz | Bz2 | Xz

TarIndex
  entries: Vec<TarEntryMeta>           # in scan order
  path_to_seq: HashMap<String, usize>  # normalized path → sequential index (for re-scan)

TarEntryMeta
  path:        String                  # normalized entry path
  size:        u64
  mtime:       SystemTime
  kind:        TarEntryKind            # File | Dir | Symlink(String) | Other
  seq_index:   usize                   # position in scan order (0-based)
```

- **VfsPath mapping**: same authority/segments convention as ZipFs; scheme = `"tar"`
- **Capabilities**: `VfsCaps::empty()`
- **Read on access**: for each `read_stream`, re-open archive, decompress and scan entries in order until reaching `seq_index`; return entry bytes
- **Write ops**: all return `VfsError::Unsupported`

---

## SftpFs (remote feature)

```
SftpFs
  authority:  SmolStr                  # "user@host:port" (normalised; port explicit)
  session:    Arc<SftpSession>         # russh_sftp::SftpSession; Clone-able
  handle:     Arc<client::Handle<HostKeyHandler>>   # SSH handle for reconnect
  config:     SftpConfig               # from cargonaut-config

SftpCredentials = PublicKey { use_agent: bool, key_path: Option<PathBuf> }
                | Password(String)

HostKeyHandler
  known_hosts_path: PathBuf
  host: String
  port: u16
  host_key_tx: Option<oneshot::Sender<HostKeyEvent>>

HostKeyEvent
  fingerprint: String
  accept_tx:   oneshot::Sender<bool>
```

- **VfsPath mapping**: `sftp://{user@host:port}/{path/segments}` — authority is the server identifier
- **Capabilities**: `SEEKABLE | RANDOM_WRITE | METADATA_RICH | ATOMIC_RENAME | SYMLINKS`
- **Connection**: one `SftpSession` per `SftpFs` instance; shared via `Arc<SftpSession>` (Clone)
- **Reconnect**: on transport error, rebuild `handle` + `session` up to 3 times (200/400/800ms backoff)
- **Host-key flow**: if `known_hosts` check fails, `HostKeyHandler` sends `HostKeyEvent` to `App` via channel; `App` shows `ActiveDialog::HostKeyVerify`; user Accept/Reject flows back via `accept_tx`

---

## FtpFs (remote feature)

```
FtpFs
  authority: SmolStr                   # "user@host:port"
  conn:      Arc<Mutex<AsyncFtpStream>>
  config:    FtpConfig

FtpConfig (in cargonaut-config)
  connect_timeout_secs: u32   # default 30
  passive_mode:         bool  # default true
```

- **VfsPath mapping**: `ftp://{user@host:port}/{path/segments}`
- **Capabilities**: `ATOMIC_RENAME` only
- **Concurrency**: all ops serialised via `Mutex` (FTP's architectural limit)

---

## State transitions: Pane backend lifecycle

```
[local: file://]
       │
       │ Enter on .zip/.tar file
       ▼
[archive: zip:// or tar://]
       │
       │ navigate_up at archive root
       ▼
[local: file://] ← restored to parent directory of archive file
```

```
[local: file://]
       │
       │ F2 → "Connect SFTP/FTP" → URL confirmed → connection established
       ▼
[remote: sftp:// or ftp://]
       │
       │ navigate_up at remote root  (optional — no mandatory return to local)
       ▼
[remote: sftp:// or ftp://] ← stays on remote; user navigates up within remote tree
```

There is no automatic transition from remote back to local; the user closes the connection by navigating to a `file://` path manually (or by opening the other pane).

---

## AppEvent extensions

```
AppEvent (existing enum, extended):
  + HostKeyVerification {
      fingerprint: String,
      accept_tx:   oneshot::Sender<bool>,
    }
```

The `App` surfaces this event to the UI, which creates `ActiveDialog::HostKeyVerify`.

---

## ActiveDialog extensions

```
ActiveDialog (existing enum, extended):
  + HostKeyVerify {
      widget:    HostKeyVerifyDialog,
      accept_tx: oneshot::Sender<bool>,
    }
  + RemoteConnect {
      kind:   RemoteKind,              # Sftp | Ftp
      widget: PathInputDialog,
    }

RemoteKind = Sftp | Ftp
```
