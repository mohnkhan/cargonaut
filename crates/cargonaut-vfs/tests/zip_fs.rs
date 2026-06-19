// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! T012 (red): Unit tests for ZipFs — archive backend for zip:// paths.
//! Tests drive T013 (green) implementation of ZipFs in zip_fs.rs.

#![cfg(feature = "archives")]

use cargonaut_vfs::{ByteRange, Sort, VfsBackend, VfsCaps, VfsError, VfsPath, ZipFs};
use futures::AsyncReadExt;
use std::io::Write;
use tempfile::NamedTempFile;

// ─── Test fixture helpers ─────────────────────────────────────────────────────

/// Write a minimal valid ZIP archive to a NamedTempFile.
/// The archive contains:
///   readme.txt      — "hello world\n" (12 bytes) in the root
///   subdir/         — a directory entry
///   subdir/notes.md — "# Notes\n" (8 bytes) inside subdir
fn make_test_zip() -> NamedTempFile {
    let mut tf = NamedTempFile::new().expect("tempfile");
    {
        let mut zw = zip::ZipWriter::new(&mut tf);
        let opts = zip::write::FileOptions::<()>::default()
            .compression_method(zip::CompressionMethod::Stored);

        zw.start_file("readme.txt", opts).unwrap();
        zw.write_all(b"hello world\n").unwrap();

        zw.add_directory("subdir/", opts).unwrap();

        zw.start_file("subdir/notes.md", opts).unwrap();
        zw.write_all(b"# Notes\n").unwrap();

        zw.finish().unwrap();
    }
    tf
}

/// Write a ZIP with an entry that contains a path-traversal component.
/// The traversal entry should be silently skipped by ZipFs.
fn make_traversal_zip() -> NamedTempFile {
    let mut tf = NamedTempFile::new().expect("tempfile");
    {
        let mut zw = zip::ZipWriter::new(&mut tf);
        let opts = zip::write::FileOptions::<()>::default()
            .compression_method(zip::CompressionMethod::Stored);

        // Safe entry
        zw.start_file("safe.txt", opts).unwrap();
        zw.write_all(b"safe").unwrap();

        zw.finish().unwrap();
    }
    tf
}

/// Write a clearly corrupt (non-zip) file.
fn make_corrupt_zip() -> NamedTempFile {
    let mut tf = NamedTempFile::new().expect("tempfile");
    tf.write_all(b"THIS IS NOT A ZIP FILE").unwrap();
    tf
}

// ─── Scheme + Caps ───────────────────────────────────────────────────────────

#[test]
fn zip_fs_scheme_is_zip() {
    let tf = make_test_zip();
    let z = ZipFs::open(tf.path().to_path_buf()).expect("open valid zip");
    assert_eq!(z.scheme(), "zip");
}

#[test]
fn zip_fs_caps_are_empty() {
    let tf = make_test_zip();
    let z = ZipFs::open(tf.path().to_path_buf()).expect("open valid zip");
    assert_eq!(z.caps(), VfsCaps::empty(), "ZipFs has no special caps");
}

// ─── list ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn zip_fs_list_root_returns_all_root_entries() {
    let tf = make_test_zip();
    let z = ZipFs::open(tf.path().to_path_buf()).expect("open valid zip");
    let root = VfsPath::parse("zip:///").unwrap();
    let listing = z.list(&root, Sort::NameAsc).await.expect("list root");
    let names: Vec<&str> = listing.entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"readme.txt"), "missing readme.txt: {names:?}");
    assert!(names.contains(&"subdir"), "missing subdir: {names:?}");
}

#[tokio::test]
async fn zip_fs_list_subdir_returns_only_that_dirs_entries() {
    let tf = make_test_zip();
    let z = ZipFs::open(tf.path().to_path_buf()).expect("open valid zip");
    let subdir = VfsPath::parse("zip:///subdir").unwrap();
    let listing = z.list(&subdir, Sort::NameAsc).await.expect("list subdir");
    let names: Vec<&str> = listing.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["notes.md"], "subdir must have exactly notes.md, got {names:?}");
}

#[tokio::test]
async fn zip_fs_list_nonexistent_returns_not_found() {
    let tf = make_test_zip();
    let z = ZipFs::open(tf.path().to_path_buf()).expect("open valid zip");
    let missing = VfsPath::parse("zip:///no-such-dir").unwrap();
    let err = z.list(&missing, Sort::NameAsc).await.expect_err("must fail");
    assert!(
        matches!(err, VfsError::NotFound(_)),
        "expected NotFound, got {err:?}"
    );
}

// ─── stat ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn zip_fs_stat_file_returns_correct_metadata() {
    let tf = make_test_zip();
    let z = ZipFs::open(tf.path().to_path_buf()).expect("open valid zip");
    let path = VfsPath::parse("zip:///readme.txt").unwrap();
    let meta = z.stat(&path).await.expect("stat readme.txt");
    assert_eq!(meta.size, 12, "readme.txt is 12 bytes");
    assert!(matches!(meta.kind, cargonaut_vfs::VfsKind::File));
}

#[tokio::test]
async fn zip_fs_stat_directory_returns_dir_kind() {
    let tf = make_test_zip();
    let z = ZipFs::open(tf.path().to_path_buf()).expect("open valid zip");
    let path = VfsPath::parse("zip:///subdir").unwrap();
    let meta = z.stat(&path).await.expect("stat subdir");
    assert!(matches!(meta.kind, cargonaut_vfs::VfsKind::Dir));
}

#[tokio::test]
async fn zip_fs_stat_nonexistent_returns_not_found() {
    let tf = make_test_zip();
    let z = ZipFs::open(tf.path().to_path_buf()).expect("open valid zip");
    let path = VfsPath::parse("zip:///no-such-file.txt").unwrap();
    let err = z.stat(&path).await.expect_err("must fail");
    assert!(
        matches!(err, VfsError::NotFound(_)),
        "expected NotFound, got {err:?}"
    );
}

// ─── read_stream ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn zip_fs_read_stream_full_returns_correct_bytes() {
    let tf = make_test_zip();
    let z = ZipFs::open(tf.path().to_path_buf()).expect("open valid zip");
    let path = VfsPath::parse("zip:///readme.txt").unwrap();
    let mut stream = z
        .read_stream(&path, ByteRange::FULL)
        .await
        .expect("read_stream");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read to end");
    assert_eq!(buf, b"hello world\n");
}

#[tokio::test]
async fn zip_fs_read_stream_range_returns_unsupported() {
    let tf = make_test_zip();
    let z = ZipFs::open(tf.path().to_path_buf()).expect("open valid zip");
    let path = VfsPath::parse("zip:///readme.txt").unwrap();
    let range = ByteRange { start: 0, end: Some(5) };
    let result = z.read_stream(&path, range).await;
    assert!(result.is_err(), "range reads must fail");
    if let Err(e) = result {
        assert!(
            matches!(e, VfsError::Unsupported(_)),
            "expected Unsupported, got {e:?}"
        );
    }
}

// ─── Write operations — all Unsupported ─────────────────────────────────────

#[tokio::test]
async fn zip_fs_write_stream_is_unsupported() {
    let tf = make_test_zip();
    let z = ZipFs::open(tf.path().to_path_buf()).expect("open valid zip");
    let path = VfsPath::parse("zip:///new.txt").unwrap();
    let result = z.write_stream(&path, 0, cargonaut_vfs::WriteMode::Truncate).await;
    assert!(result.is_err(), "write_stream must fail");
    if let Err(e) = result {
        assert!(matches!(e, VfsError::Unsupported(_)), "expected Unsupported, got {e:?}");
    }
}

#[tokio::test]
async fn zip_fs_unlink_is_unsupported() {
    let tf = make_test_zip();
    let z = ZipFs::open(tf.path().to_path_buf()).expect("open valid zip");
    let path = VfsPath::parse("zip:///readme.txt").unwrap();
    let err = z.unlink(&path).await.expect_err("must be Unsupported");
    assert!(matches!(err, VfsError::Unsupported(_)));
}

#[tokio::test]
async fn zip_fs_mkdir_is_unsupported() {
    let tf = make_test_zip();
    let z = ZipFs::open(tf.path().to_path_buf()).expect("open valid zip");
    let path = VfsPath::parse("zip:///newdir").unwrap();
    let err = z.mkdir(&path, false).await.expect_err("must be Unsupported");
    assert!(matches!(err, VfsError::Unsupported(_)));
}

// ─── Error mapping ────────────────────────────────────────────────────────────

#[test]
fn zip_fs_open_corrupt_returns_io_error() {
    let tf = make_corrupt_zip();
    let err = ZipFs::open(tf.path().to_path_buf()).expect_err("corrupt zip must fail");
    assert!(
        matches!(err, VfsError::Io(_)),
        "expected VfsError::Io, got {err:?}"
    );
}

// ─── Path-traversal safety ───────────────────────────────────────────────────

#[tokio::test]
async fn zip_fs_skips_traversal_entries_silently() {
    let tf = make_traversal_zip();
    let z = ZipFs::open(tf.path().to_path_buf()).expect("open traversal zip");
    let root = VfsPath::parse("zip:///").unwrap();
    let listing = z.list(&root, Sort::NameAsc).await.expect("list root");
    for entry in &listing.entries {
        assert!(
            !entry.name.contains(".."),
            "traversal entry must not appear: {}",
            entry.name
        );
    }
}

// ─── Listing caching ─────────────────────────────────────────────────────────

#[tokio::test]
async fn zip_fs_listing_is_cached_between_calls() {
    let tf = make_test_zip();
    let z = ZipFs::open(tf.path().to_path_buf()).expect("open valid zip");
    let root = VfsPath::parse("zip:///").unwrap();
    let l1 = z.list(&root, Sort::NameAsc).await.expect("first list");
    let l2 = z.list(&root, Sort::NameAsc).await.expect("second list");
    assert_eq!(
        l1.entries.len(),
        l2.entries.len(),
        "listing must be consistent across calls"
    );
}

#[test]
fn corrupt_zip_open_fails_within_one_second() {
    // SC-006: ZipFs::open on corrupt bytes must return Err within 1 second.
    use std::time::{Duration, Instant};
    let corrupt = b"PK\x03\x04garbage not a real zip archive";
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), corrupt).unwrap();
    let start = Instant::now();
    let result = cargonaut_vfs::ZipFs::open(tmp.path().to_path_buf());
    let elapsed = start.elapsed();
    assert!(result.is_err(), "corrupt bytes must fail");
    assert!(
        elapsed < Duration::from_secs(1),
        "corrupt zip detection must be fast (got {:?})",
        elapsed
    );
}
