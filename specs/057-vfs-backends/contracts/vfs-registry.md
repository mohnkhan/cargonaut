# Contract: VfsRegistry

**Crate**: `cargonaut-vfs`
**Type**: `pub struct VfsRegistry`

## Purpose

`VfsRegistry` maps URI scheme strings (and optionally scheme+authority pairs) to `Arc<dyn VfsBackend>` instances, enabling `App` to dispatch pane operations to the correct backend without any `match` on schemes scattered across the codebase.

## API

```rust
impl VfsRegistry {
    /// Create a new registry. `local` is the LocalFs backend, registered
    /// under scheme "file". It is always present; `resolve` never returns
    /// `None` for `file://` paths.
    pub fn new(local: Arc<dyn VfsBackend>) -> Self;

    /// Return the local filesystem backend (`file://`).
    pub fn local(&self) -> Arc<dyn VfsBackend>;

    /// Register a connection-scoped backend. Key must be `"{scheme}://{authority}"`.
    /// Overwrites any prior registration for the same key.
    pub fn register_remote(&mut self, key: impl Into<SmolStr>, backend: Arc<dyn VfsBackend>);

    /// Resolve the backend for `path`.
    ///
    /// Lookup order:
    ///   1. If `path.authority.is_some()`: check `remote_map["{scheme}://{authority}"]`
    ///   2. If `path.scheme == "file"`: return `local()`
    ///   3. Otherwise: `None` (caller must surface an error)
    pub fn resolve(&self, path: &VfsPath) -> Option<Arc<dyn VfsBackend>>;
}
```

## Invariants

- `registry.local()` always returns `Some`; it is set at construction and never removed.
- `resolve` is deterministic: same `path` always returns the same `Arc` within a registry instance.
- Remote backends registered with the same key overwrite the previous entry (reconnect scenario).
- Archive backends (`ZipFs`, `TarFs`) are **not** stored in the registry; they live in `PaneState.backend` and are dropped when the pane navigates away.

## Error cases

- `resolve` returns `None` for unregistered schemes. Callers MUST propagate this as an appropriate `AppError` (e.g., `BackendNotFound`).

## Object-safety

`VfsRegistry` itself is a concrete type, not a trait. It holds `Arc<dyn VfsBackend>`, which is object-safe per the existing `tests/dyn_dispatch.rs` test.
