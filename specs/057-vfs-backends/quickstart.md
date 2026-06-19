# Quickstart Validation Guide: Feature 057 — VFS Backends

This guide documents runnable validation scenarios that prove the feature works end-to-end. Prerequisites, commands, and expected outcomes are listed for each scenario.

## Prerequisites

```bash
# Verify tmpfs is active
make tmpfs-status

# Build the project
make build

# Run all tests (should pass without modification at any point in development)
cargo test --workspace
```

For SFTP scenarios: a local SFTP server is required (e.g. `sftp-server` from OpenSSH, or a Docker container running `atmoz/sftp`). For FTP: `vsftpd` or `pure-ftpd` locally, or a Docker container.

---

## Scenario 1: Browse a ZIP archive (US1, SC-001, SC-002)

**Setup**:
```bash
# Create a test ZIP with nested structure
cd /tmp
mkdir -p test-content/subdir
echo "hello world" > test-content/subdir/file.txt
echo "data" > test-content/root.txt
zip -r test.zip test-content/
```

**Validation**:
1. Start `cargonaut` with left pane pointing to `/tmp`
2. Navigate cursor to `test.zip`
3. Press **Enter**
4. **Expected**: Pane header updates to `zip://tmp%2Ftest.zip/`; entries `test-content/` shown
5. Navigate into `test-content/subdir/`
6. **Expected**: `file.txt` shown with size matching `hello world\n` (12 bytes)
7. Press **F5** with right pane set to `/tmp/out/` (create it first: `mkdir /tmp/out`)
8. **Expected**: `/tmp/out/file.txt` created with content `hello world`; checksum matches
9. Navigate to `..` until at archive root, then `..` again
10. **Expected**: Pane returns to `/tmp` (local file:// backend)

**Timing**: Steps 3-4 should complete within 500 ms for this archive (SC-001).

---

## Scenario 2: Browse a TAR.GZ archive (US2, SC-001, SC-002)

**Setup**:
```bash
cd /tmp
mkdir -p tartest/src
echo 'fn main() {}' > tartest/src/main.rs
echo '# Readme' > tartest/README.md
tar -czf tartest.tar.gz tartest/
```

**Validation**:
1. Navigate left pane to `/tmp`
2. Cursor on `tartest.tar.gz`, press **Enter**
3. **Expected**: Pane shows `tar://tmp%2Ftartest.tar.gz/` with `tartest/` entry
4. Navigate into `tartest/src/`
5. **Expected**: `main.rs` shown
6. Copy `main.rs` to right local pane; verify content matches

**Repeat** for `.tar.bz2` and `.tar.xz` (same test archive compressed differently).

---

## Scenario 3: Path traversal is rejected (security gate)

**Setup**:
```bash
# Create a malicious TAR with a path traversal entry
python3 -c "
import tarfile, io
tf = tarfile.open('/tmp/evil.tar', 'w')
info = tarfile.TarInfo(name='../etc/evil')
info.size = 5
tf.addfile(info, io.BytesIO(b'pwned'))
tf.close()
"
```

**Validation**:
1. Navigate to `/tmp`, press Enter on `evil.tar`
2. **Expected**: Archive opens; `../etc/evil` entry is NOT listed (skipped silently)
3. **Expected**: `/etc/evil` was not created on disk

---

## Scenario 4: Corrupt archive shows error banner (SC-006)

```bash
echo "this is not a zip" > /tmp/fake.zip
```

1. Navigate to `/tmp`, press Enter on `fake.zip`
2. **Expected**: Error banner appears ("Cannot open archive: …"); no crash; pane remains navigable

---

## Scenario 5: SFTP connect and transfer (US3, SC-003, SC-004)

**Setup** (using Docker):
```bash
docker run -d --name sftp-test -p 2222:22 \
  -v /tmp/sftp-test:/home/testuser/upload \
  atmoz/sftp testuser:testpass:1001
mkdir -p /tmp/sftp-test
echo "transfer me" > /tmp/sftp-test/sample.txt
```

**Validation**:
1. Press **F2** to open User Menu
2. Select **"Connect SFTP…"**
3. **Expected**: `PathInputDialog` opens pre-filled with `sftp://user@host/`
4. Edit to `sftp://testuser@localhost:2222/upload`; press Enter
5. **Expected**: "Connecting…" shown in pane header; within 3 s, pane shows `/upload` contents including `sample.txt` (SC-003)
6. Copy `sample.txt` to the local pane
7. **Expected**: Transfer completes; `sha256sum` of source and dest match (SC-002/SC-004)

**SSH host-key scenario** (first connect to a new host):
- Clear `~/.ssh/known_hosts` or use an unknown host
- Connect to the SFTP server
- **Expected**: Modal dialog showing host fingerprint; "Accept" / "Reject" buttons
- Press Accept
- **Expected**: Connection proceeds; key added to `~/.ssh/known_hosts`

---

## Scenario 6: SFTP auth failure banner (SC-007)

1. Try connecting with wrong password: `sftp://testuser@localhost:2222/` with password `wrong`
2. **Expected**: After up to 3 retry attempts (watch `~/.local/share/cargonaut/debug.log` for `WARN` entries), error banner appears in pane; app stable

---

## Scenario 7: FTP connect and browse (US4)

**Setup**:
```bash
docker run -d --name ftp-test -p 21:21 -p 21100-21110:21100-21110 \
  -e FTP_USER=ftpuser -e FTP_PASS=ftppass \
  -v /tmp/ftp-test:/home/vsftpd/ftpuser \
  garethflowers/ftp-server
mkdir -p /tmp/ftp-test
echo "ftp file" > /tmp/ftp-test/hello.txt
```

**Validation**:
1. F2 → "Connect FTP…" → edit URL to `ftp://ftpuser@localhost/`; press Enter
2. **Expected**: Pane shows FTP root with `hello.txt`
3. Copy `hello.txt` to local pane; verify content

---

## Scenario 8: Binary size check (SC-008)

```bash
# Default build (both features)
cargo build --release 2>/dev/null
ls -l target/release/cargonaut | awk '{print $5}'

# No-features build
cargo build --release --no-default-features 2>/dev/null
ls -l target/release/cargonaut | awk '{print $5}'

# Delta must be ≤ 1,500,000 bytes
bash scripts/check-binary-size.sh
```

---

## Scenario 9: Regression check (SC-005)

```bash
cargo test --workspace
# Expected: all pre-existing tests pass; zero failures in file:// paths
```

---

## Ongoing CI checks

| Check | Command | Gate |
|---|---|---|
| Archive listing bench | `cargo bench --bench archive_listing` | ≤500 ms / 10k entries |
| Binary size | `bash scripts/check-binary-size.sh` | ≤baseline + 1.5 MiB |
| Unit tests | `cargo test --workspace` | Zero failures |
| Clippy | `cargo clippy --workspace --all-targets -- -D warnings` | Zero warnings |
| Docs | `RUSTDOCFLAGS="-D rustdoc::broken-intra-doc-links" cargo doc --workspace` | Zero errors |
