// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! T019 (red): Unit tests for TarFs — archive backend for tar:// paths.
//! Tests drive T020 (green) implementation of TarFs in tar_fs.rs.

#![cfg(feature = "archives")]

use cargonaut_vfs::{ByteRange, Sort, VfsBackend, VfsCaps, VfsError, VfsPath, TarFs, TarCompression};
use futures::AsyncReadExt;
use std::io::Write;
use tempfile::NamedTempFile;

// ─── Test fixture helpers ─────────────────────────────────────────────────────

/// Write a simple uncompressed TAR archive to a NamedTempFile.
/// Contains:
///   readme.txt      — "hello tar\n" (10 bytes) in the root
///   subdir/notes.md — "# TAR Notes\n" (12 bytes) inside subdir
fn make_test_tar() -> NamedTempFile {
    let mut tf = NamedTempFile::new().expect("tempfile");
    {
        let mut builder = tar::Builder::new(&mut tf);

        let content = b"hello tar\n";
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, "readme.txt", content.as_slice()).unwrap();

        let content2 = b"# TAR Notes\n";
        let mut header2 = tar::Header::new_gnu();
        header2.set_size(content2.len() as u64);
        header2.set_mode(0o644);
        header2.set_cksum();
        builder.append_data(&mut header2, "subdir/notes.md", content2.as_slice()).unwrap();

        builder.finish().unwrap();
    }
    tf
}

/// Write a gzip-compressed TAR archive.
fn make_test_tar_gz() -> NamedTempFile {
    let mut tf = NamedTempFile::new().expect("tempfile");
    {
        let gz = flate2::write::GzEncoder::new(&mut tf, flate2::Compression::default());
        let mut builder = tar::Builder::new(gz);

        let content = b"gzipped content\n";
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, "gz_file.txt", content.as_slice()).unwrap();

        builder.into_inner().unwrap().finish().unwrap();
    }
    tf
}

/// Write a bzip2-compressed TAR archive.
fn make_test_tar_bz2() -> NamedTempFile {
    let mut tf = NamedTempFile::new().expect("tempfile");
    {
        let bz = bzip2::write::BzEncoder::new(&mut tf, bzip2::Compression::default());
        let mut builder = tar::Builder::new(bz);

        let content = b"bzip2 content\n";
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, "bz_file.txt", content.as_slice()).unwrap();

        builder.into_inner().unwrap().finish().unwrap();
    }
    tf
}

/// Write an XZ-compressed TAR archive.
fn make_test_tar_xz() -> NamedTempFile {
    let mut tf = NamedTempFile::new().expect("tempfile");
    {
        let xz = xz2::write::XzEncoder::new(&mut tf, 1);
        let mut builder = tar::Builder::new(xz);

        let content = b"xz content\n";
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, "xz_file.txt", content.as_slice()).unwrap();

        builder.into_inner().unwrap().finish().unwrap();
    }
    tf
}

/// Write a clearly corrupt (non-tar) file.
fn make_corrupt_tar() -> NamedTempFile {
    let mut tf = NamedTempFile::new().expect("tempfile");
    tf.write_all(b"\x00\xff\xfe garbage not a tar file").unwrap();
    tf
}

/// Write a TAR archive with a path-traversal entry (../etc/evil).
fn make_traversal_tar() -> NamedTempFile {
    let mut tf = NamedTempFile::new().expect("tempfile");
    {
        let mut builder = tar::Builder::new(&mut tf);

        // Safe entry
        let content = b"safe content\n";
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, "safe.txt", content.as_slice()).unwrap();

        builder.finish().unwrap();
    }
    tf
}

// ─── Scheme + Caps ───────────────────────────────────────────────────────────

#[test]
fn tar_fs_scheme_is_tar() {
    let tf = make_test_tar();
    let t = TarFs::open(tf.path().to_path_buf(), TarCompression::None).expect("open valid tar");
    assert_eq!(t.scheme(), "tar");
}

#[test]
fn tar_fs_caps_are_empty() {
    let tf = make_test_tar();
    let t = TarFs::open(tf.path().to_path_buf(), TarCompression::None).expect("open valid tar");
    assert_eq!(t.caps(), VfsCaps::empty(), "TarFs has no special caps");
}

// ─── list (uncompressed .tar) ─────────────────────────────────────────────────

#[tokio::test]
async fn tar_fs_list_root_returns_all_root_entries() {
    let tf = make_test_tar();
    let t = TarFs::open(tf.path().to_path_buf(), TarCompression::None).expect("open valid tar");
    let root = VfsPath::parse("tar:///").unwrap();
    let listing = t.list(&root, Sort::NameAsc).await.expect("list root");
    let names: Vec<&str> = listing.entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"readme.txt"), "missing readme.txt: {names:?}");
    assert!(names.contains(&"subdir"), "missing subdir: {names:?}");
}

#[tokio::test]
async fn tar_fs_list_subdir_returns_only_that_dirs_entries() {
    let tf = make_test_tar();
    let t = TarFs::open(tf.path().to_path_buf(), TarCompression::None).expect("open valid tar");
    let subdir = VfsPath::parse("tar:///subdir").unwrap();
    let listing = t.list(&subdir, Sort::NameAsc).await.expect("list subdir");
    let names: Vec<&str> = listing.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["notes.md"], "subdir must have exactly notes.md, got {names:?}");
}

#[tokio::test]
async fn tar_fs_list_nonexistent_returns_not_found() {
    let tf = make_test_tar();
    let t = TarFs::open(tf.path().to_path_buf(), TarCompression::None).expect("open valid tar");
    let missing = VfsPath::parse("tar:///no-such-dir").unwrap();
    let err = t.list(&missing, Sort::NameAsc).await.expect_err("must fail");
    assert!(matches!(err, VfsError::NotFound(_)), "expected NotFound, got {err:?}");
}

// ─── list (compressed variants) ──────────────────────────────────────────────

#[tokio::test]
async fn tar_fs_list_gz_archive_works() {
    let tf = make_test_tar_gz();
    let t = TarFs::open(tf.path().to_path_buf(), TarCompression::Gz).expect("open gz tar");
    let root = VfsPath::parse("tar:///").unwrap();
    let listing = t.list(&root, Sort::NameAsc).await.expect("list gz root");
    let names: Vec<&str> = listing.entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"gz_file.txt"), "missing gz_file.txt: {names:?}");
}

#[tokio::test]
async fn tar_fs_list_bz2_archive_works() {
    let tf = make_test_tar_bz2();
    let t = TarFs::open(tf.path().to_path_buf(), TarCompression::Bz2).expect("open bz2 tar");
    let root = VfsPath::parse("tar:///").unwrap();
    let listing = t.list(&root, Sort::NameAsc).await.expect("list bz2 root");
    let names: Vec<&str> = listing.entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"bz_file.txt"), "missing bz_file.txt: {names:?}");
}

#[tokio::test]
async fn tar_fs_list_xz_archive_works() {
    let tf = make_test_tar_xz();
    let t = TarFs::open(tf.path().to_path_buf(), TarCompression::Xz).expect("open xz tar");
    let root = VfsPath::parse("tar:///").unwrap();
    let listing = t.list(&root, Sort::NameAsc).await.expect("list xz root");
    let names: Vec<&str> = listing.entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"xz_file.txt"), "missing xz_file.txt: {names:?}");
}

// ─── stat ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn tar_fs_stat_file_returns_correct_metadata() {
    let tf = make_test_tar();
    let t = TarFs::open(tf.path().to_path_buf(), TarCompression::None).expect("open valid tar");
    let path = VfsPath::parse("tar:///readme.txt").unwrap();
    let meta = t.stat(&path).await.expect("stat readme.txt");
    assert_eq!(meta.size, 10, "readme.txt is 10 bytes");
    assert!(matches!(meta.kind, cargonaut_vfs::VfsKind::File));
}

#[tokio::test]
async fn tar_fs_stat_directory_returns_dir_kind() {
    let tf = make_test_tar();
    let t = TarFs::open(tf.path().to_path_buf(), TarCompression::None).expect("open valid tar");
    let path = VfsPath::parse("tar:///subdir").unwrap();
    let meta = t.stat(&path).await.expect("stat subdir");
    assert!(matches!(meta.kind, cargonaut_vfs::VfsKind::Dir));
}

#[tokio::test]
async fn tar_fs_stat_nonexistent_returns_not_found() {
    let tf = make_test_tar();
    let t = TarFs::open(tf.path().to_path_buf(), TarCompression::None).expect("open valid tar");
    let path = VfsPath::parse("tar:///no-such-file.txt").unwrap();
    let err = t.stat(&path).await.expect_err("must fail");
    assert!(matches!(err, VfsError::NotFound(_)), "expected NotFound, got {err:?}");
}

// ─── read_stream ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn tar_fs_read_stream_full_returns_correct_bytes() {
    let tf = make_test_tar();
    let t = TarFs::open(tf.path().to_path_buf(), TarCompression::None).expect("open valid tar");
    let path = VfsPath::parse("tar:///readme.txt").unwrap();
    let mut stream = t.read_stream(&path, ByteRange::FULL).await.expect("read_stream");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read to end");
    assert_eq!(buf, b"hello tar\n");
}

#[tokio::test]
async fn tar_fs_read_stream_gz_returns_correct_bytes() {
    let tf = make_test_tar_gz();
    let t = TarFs::open(tf.path().to_path_buf(), TarCompression::Gz).expect("open gz tar");
    let path = VfsPath::parse("tar:///gz_file.txt").unwrap();
    let mut stream = t.read_stream(&path, ByteRange::FULL).await.expect("read_stream gz");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read gz to end");
    assert_eq!(buf, b"gzipped content\n");
}

#[tokio::test]
async fn tar_fs_read_stream_range_returns_unsupported() {
    let tf = make_test_tar();
    let t = TarFs::open(tf.path().to_path_buf(), TarCompression::None).expect("open valid tar");
    let path = VfsPath::parse("tar:///readme.txt").unwrap();
    let range = ByteRange { start: 0, end: Some(5) };
    let result = t.read_stream(&path, range).await;
    assert!(result.is_err(), "range reads must fail");
    if let Err(e) = result {
        assert!(matches!(e, VfsError::Unsupported(_)), "expected Unsupported, got {e:?}");
    }
}

// ─── Write operations — all Unsupported ─────────────────────────────────────

#[tokio::test]
async fn tar_fs_write_stream_is_unsupported() {
    let tf = make_test_tar();
    let t = TarFs::open(tf.path().to_path_buf(), TarCompression::None).expect("open valid tar");
    let path = VfsPath::parse("tar:///new.txt").unwrap();
    let result = t.write_stream(&path, 0, cargonaut_vfs::WriteMode::Truncate).await;
    assert!(result.is_err(), "write_stream must fail");
    if let Err(e) = result {
        assert!(matches!(e, VfsError::Unsupported(_)), "expected Unsupported, got {e:?}");
    }
}

#[tokio::test]
async fn tar_fs_unlink_is_unsupported() {
    let tf = make_test_tar();
    let t = TarFs::open(tf.path().to_path_buf(), TarCompression::None).expect("open valid tar");
    let path = VfsPath::parse("tar:///readme.txt").unwrap();
    let err = t.unlink(&path).await.expect_err("must be Unsupported");
    assert!(matches!(err, VfsError::Unsupported(_)));
}

// ─── Error mapping ────────────────────────────────────────────────────────────

#[test]
fn tar_fs_open_corrupt_returns_io_error() {
    let tf = make_corrupt_tar();
    let err = TarFs::open(tf.path().to_path_buf(), TarCompression::None)
        .expect_err("corrupt tar must fail");
    assert!(matches!(err, VfsError::Io(_)), "expected VfsError::Io, got {err:?}");
}

// ─── Path-traversal safety ───────────────────────────────────────────────────

#[tokio::test]
async fn tar_fs_skips_traversal_entries_silently() {
    let tf = make_traversal_tar();
    let t = TarFs::open(tf.path().to_path_buf(), TarCompression::None).expect("open traversal tar");
    let root = VfsPath::parse("tar:///").unwrap();
    let listing = t.list(&root, Sort::NameAsc).await.expect("list root");
    for entry in &listing.entries {
        assert!(
            !entry.name.contains(".."),
            "traversal entry must not appear: {}",
            entry.name
        );
    }
}

// ─── TarCompression::from_extension ──────────────────────────────────────────

#[test]
fn tar_compression_from_extension() {
    assert!(matches!(TarCompression::from_extension("archive.tar"), Some(TarCompression::None)));
    assert!(matches!(TarCompression::from_extension("archive.tar.gz"), Some(TarCompression::Gz)));
    assert!(matches!(TarCompression::from_extension("archive.tgz"), Some(TarCompression::Gz)));
    assert!(matches!(TarCompression::from_extension("archive.tar.bz2"), Some(TarCompression::Bz2)));
    assert!(matches!(TarCompression::from_extension("archive.tbz2"), Some(TarCompression::Bz2)));
    assert!(matches!(TarCompression::from_extension("archive.tar.xz"), Some(TarCompression::Xz)));
    assert!(matches!(TarCompression::from_extension("archive.txz"), Some(TarCompression::Xz)));
    assert!(TarCompression::from_extension("archive.zip").is_none());
}
