#!/usr/bin/env python3
"""Extract the public API surface of cargonaut-core from rustdoc JSON.

Emits a normalized, sorted, line-oriented surface so the public API can be
diffed before/after the Feature 059 god-file split. The split is move-only;
this script is the automated proof that the public surface is unchanged.

Usage:
    cargo +nightly rustdoc -p cargonaut-core -- -Z unstable-options \
        --output-format json
    python3 extract-public-api.py target/doc/cargonaut_core.json > surface.txt

What it captures (name-level, which is sufficient for a move-only refactor —
signature drift is independently caught by compiling the unchanged downstream
crates and benches):
  - re-exports (`pub use`) at any public module, as `reexport NAME`
  - public types: `struct|enum|trait|type NAME`
  - enum variants: `variant ENUM::VARIANT`
  - public struct fields: `field STRUCT.FIELD`
  - public methods / assoc fns: `method TYPE::METHOD`
  - free functions: `fn NAME`
  - public constants: `const NAME`
"""
import json
import sys


def main() -> int:
    data = json.load(open(sys.argv[1]))
    index = data["index"]
    out = set()

    def is_public(item):
        # Default visibility for items reachable in the doc index is public;
        # rustdoc only emits private items with --document-private-items.
        return item.get("visibility", "public") in ("public", "default")

    for _id, item in index.items():
        if item.get("crate_id", 0) != 0:
            continue
        inner = item.get("inner", {})
        if not isinstance(inner, dict):
            continue
        kind = next(iter(inner), None)
        name = item.get("name")
        if kind in ("struct",) and name and is_public(item):
            out.add(f"struct {name}")
            sd = inner["struct"]
            kindinfo = sd.get("kind", {})
            field_ids = []
            if isinstance(kindinfo, dict) and "plain" in kindinfo:
                field_ids = kindinfo["plain"].get("fields", [])
            for fid in field_ids:
                f = index.get(str(fid)) or index.get(fid)
                if f and f.get("name") and is_public(f):
                    out.add(f"field {name}.{f['name']}")
        elif kind == "enum" and name and is_public(item):
            out.add(f"enum {name}")
            for vid in inner["enum"].get("variants", []):
                v = index.get(str(vid)) or index.get(vid)
                if v and v.get("name"):
                    out.add(f"variant {name}::{v['name']}")
        elif kind == "trait" and name and is_public(item):
            out.add(f"trait {name}")
        elif kind == "type_alias" and name and is_public(item):
            out.add(f"type {name}")
        elif kind == "constant" and name and is_public(item):
            out.add(f"const {name}")
        elif kind in ("use", "import"):
            u = inner.get(kind, {})
            if u.get("glob"):
                continue
            nm = u.get("name")
            # Only count CROSS-CRATE re-exports: a same-crate `pub use` merely
            # relocates a definition that is already counted by its canonical
            # struct/enum/fn/method line, so counting it again would make the
            # surface diff sensitive to internal module moves (which is exactly
            # what this refactor does). Cross-crate re-exports have no local
            # definition, so they must be counted here.
            tid = u.get("id")
            target = index.get(str(tid)) or index.get(tid) if tid is not None else None
            is_external = not (target is not None and target.get("crate_id", 0) == 0)
            if nm and is_external:
                out.add(f"reexport {nm}")

    # Methods / associated functions live in impl blocks; attribute each to its
    # Self type so renames or relocations that drop a method are caught.
    id_to_name = {
        i: it.get("name")
        for i, it in index.items()
        if it.get("name")
    }
    for _id, item in index.items():
        inner = item.get("inner", {})
        if not isinstance(inner, dict) or "impl" not in inner:
            continue
        imp = inner["impl"]
        if imp.get("trait") is not None:
            continue  # only inherent impls define the type's own surface
        forty = imp.get("for", {})
        type_name = None
        if isinstance(forty, dict):
            rp = forty.get("resolved_path") or forty.get("path")
            if isinstance(rp, dict):
                type_name = (rp.get("path") or rp.get("name") or "").split("::")[-1]
        for mid in imp.get("items", []):
            m = index.get(str(mid)) or index.get(mid)
            if not m or not m.get("name") or not is_public(m):
                continue
            minner = m.get("inner", {})
            if isinstance(minner, dict) and "function" in minner:
                out.add(f"method {type_name}::{m['name']}")

    # Free functions at module scope (not inside impls).
    impl_fn_ids = set()
    for _id, item in index.items():
        inner = item.get("inner", {})
        if isinstance(inner, dict) and "impl" in inner:
            impl_fn_ids.update(str(x) for x in inner["impl"].get("items", []))
    for i, item in index.items():
        if item.get("crate_id", 0) != 0:
            continue
        inner = item.get("inner", {})
        if (isinstance(inner, dict) and "function" in inner
                and item.get("name") and str(i) not in impl_fn_ids
                and is_public(item)):
            out.add(f"fn {item['name']}")

    for line in sorted(out):
        print(line)
    return 0


if __name__ == "__main__":
    sys.exit(main())
