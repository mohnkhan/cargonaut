# Contract: Archive VfsPath Encoding

## Problem

`VfsPath` uses `scheme://authority/seg1/seg2` URI form. For archive backends, the URI must encode two distinct pieces of information:
1. The host-filesystem path to the archive file (e.g. `/home/user/archive.zip`)
2. The in-archive entry path (e.g. `subdir/file.txt`)

These must be separable without ambiguity and without changing the `VfsPath` data model.

## Encoding Convention

| Component | Contains |
|---|---|
| `scheme` | `"zip"` or `"tar"` |
| `authority` | Archive host-filesystem path, with `/` percent-encoded as `%2F` |
| `segments` | In-archive entry path (one segment per path component) |

### Examples

| Archive on disk | In-archive entry | Encoded VfsPath |
|---|---|---|
| `/home/user/archive.zip` | (root) | `zip://home%2Fuser%2Farchive.zip/` |
| `/home/user/archive.zip` | `subdir/file.txt` | `zip://home%2Fuser%2Farchive.zip/subdir/file.txt` |
| `/tmp/src.tar.gz` | `src/main.rs` | `tar://tmp%2Fsrc.tar.gz/src/main.rs` |
| `/tmp/src.tar.gz` | (root) | `tar://tmp%2Fsrc.tar.gz/` |

### Encoding rules (call site → VfsPath construction)

```rust
// To build a VfsPath pointing at the root of an archive:
let authority = host_path
    .to_str()
    .expect("non-UTF8 paths unsupported")
    .trim_start_matches('/')   // strip leading /
    .replace('/', "%2F");
let vfs_path = VfsPath {
    scheme: SmolStr::new("zip"),     // or "tar"
    authority: Some(SmolStr::new(&authority)),
    segments: SmallVec::new(),       // empty = archive root
};

// To build a VfsPath for an entry inside the archive:
let vfs_path = vfs_root.join(entry_dir).join(entry_name);
// (join() appends one segment at a time; multiple joins for sub-paths)
```

### Decoding rules (backend → host path)

```rust
// VfsPath::decode_authority() (new helper) handles this:
let host_path_str = path.decode_authority().expect("zip/tar path must have authority");
let host_path = PathBuf::from("/").join(&host_path_str);
// → /home/user/archive.zip (for authority = "home%2Fuser%2Farchive.zip")
```

## Invariants

- The `authority` of an archive `VfsPath` MUST contain only one level of percent-encoding: `/` → `%2F`. Other characters are not percent-encoded.
- `authority` is NEVER empty for archive `VfsPath`s (it always holds the archive file path).
- `segments` for the archive root are empty (`segments.len() == 0`).
- `VfsPath::join()` and `VfsPath::parent()` operate on `segments` only; the `authority` is unchanged by these operations. This means navigating up within an archive reduces `segments` without touching `authority`.
- Navigating to `parent()` when `segments.is_empty()` returns `None` — the UI detects this and transitions the pane back to the local filesystem parent directory.

## Rationale for authority-based encoding

The alternative (encoding everything in segments) would require the backend to scan segments for the first one ending in `.zip`/`.tar` to find the boundary. This heuristic fails for directories named `foo.zip/` or archives nested in such directories. The authority-based encoding is unambiguous at parse time.

## `VfsPath::decode_authority()` helper

```rust
/// Percent-decode the `authority` field, replacing `%2F` with `/`.
/// Returns `None` if the authority is absent.
pub fn decode_authority(&self) -> Option<String> {
    self.authority.as_ref().map(|a| a.replace("%2F", "/").replace("%2f", "/"))
}
```

Full URL percent-decoding (all `%XX` sequences) is applied for correctness, but in practice only `%2F` appears in archive authority values produced by this codebase.
