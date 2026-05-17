# Plugin API Contract

**Status**: Phase 3. Frozen interface (WIT). Breaking changes require a major-version bump on the plugin host.

## Interface (WIT)

```wit
package cargonaut:plugin@0.1.0;

interface host {
    /// Capabilities granted to this plugin instance by the user's config.
    record capabilities {
        read-dirs: list<string>,    // VFS URIs allowlist for read access
        read-files: bool,
        write-files: bool,
        network: bool,
    }
    log-info: func(message: string);
    log-warn: func(message: string);
    log-error: func(message: string);
    get-capabilities: func() -> capabilities;
    /// Read a directory; capability-checked.
    list-dir: func(uri: string) -> result<list<file-entry>, plugin-error>;
    record file-entry {
        name: string,
        size: u64,
        mtime: u64,         // epoch seconds
        is-dir: bool,
    }
    /// Read a file at most max-bytes; capability-checked.
    read-file: func(uri: string, max-bytes: u64) -> result<list<u8>, plugin-error>;
    variant plugin-error {
        capability-denied(string),
        not-found,
        io-error(string),
        invalid-uri(string),
    }
}

world plugin {
    import host;

    /// Called once at plugin load. Plugin returns its declared capability needs.
    /// The host enforces a subset based on user config.
    export init: func() -> capability-request;
    record capability-request {
        wants-read-dirs: list<string>,
        wants-read-files: bool,
        wants-write-files: bool,
        wants-network: bool,
    }

    /// Called for every directory listing the user views, IF the dir is
    /// in the plugin's granted read-dirs allowlist.
    /// Returns extra columns to display per file entry.
    export render-column: func(dir-uri: string, entry: host.file-entry) -> result<string, host.plugin-error>;

    /// Called on plugin shutdown (config reload, etc.). Plugin should
    /// release any held resources.
    export shutdown: func();
}
```

## Capability semantics

| Capability | What it grants |
|---|---|
| `read-dirs: [uri1, uri2, ...]` | `list-dir` works for any URI under one of these prefixes; otherwise `capability-denied`. |
| `read-files: bool` | `read-file` works for any URI within an allowed `read-dirs` prefix (combined check). |
| `write-files: bool` | A future export `write-file` works similarly. NOT in v0.1. |
| `network: bool` | A future WASI socket import is enabled. NOT in v0.1. |

**Enforcement**: The host checks capabilities BEFORE every import call. Denials log an audit entry and increment a per-plugin `denials` counter visible via `cargonaut list-plugins`.

## Plugin manifest (`plugin.toml`)

Every plugin ships with a manifest that the host parses BEFORE loading the .wasm module:

```toml
name = "git-status"
version = "0.1.0"
author = "Cargonaut Project"
description = "Show per-file git status (M/A/?/blank) as a pane column"
license = "MIT OR Apache-2.0"

# What the plugin declares it needs. The user's config can grant SUBSET only.
[capabilities-requested]
read-dirs = ["**/.git/**"]   # glob over VFS URIs
read-files = true
write-files = false
network = false

[runtime]
wasm-module = "git-status.wasm"
component-model = "0.1.0"
```

## Threat model (excerpt)

| Threat | Mitigation |
|---|---|
| Plugin reads `/etc/passwd` outside its capability | Capability check on every `list-dir`/`read-file`; out-of-cap → `capability-denied`. |
| Plugin exhausts memory | wasmtime memory limit per instance (default 64 MiB). |
| Plugin exhausts CPU (infinite loop) | wasmtime fuel limit per host call (default 10⁹ fuel ≈ ~100 ms wall time). |
| Plugin tries WASI socket (network) | WASI socket interface NOT imported unless `network = true`. |
| Plugin tries WASI subprocess (exec) | WASI subprocess interface NOT imported in any cap set. |
| Plugin attempts to escape sandbox via malformed component | wasmtime validates the component on load; rejected modules logged. Fuzzed in CI (SC-006). |
| User installs a malicious plugin | Plugin manifest requires explicit user opt-in via config; first run shows requested capabilities prominently. |
| Replay attack: plugin writes garbage to audit log | Plugins cannot write to audit log directly; host writes structured `PluginEvent` audit entries (HMAC-chained per FR-304). |

Full threat model in `security/threat-model.md` (Phase 3 deliverable).
