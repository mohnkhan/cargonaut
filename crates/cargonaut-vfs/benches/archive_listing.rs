// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! SC-001 gate: ZipFs::list(root) on a 10 000-entry archive must complete in < 500 ms.
use cargonaut_vfs::{Sort, VfsBackend, VfsPath, ZipFs};
use criterion::{criterion_group, criterion_main, Criterion};
use std::io::Write;
use std::time::Duration;
use tempfile::NamedTempFile;

fn build_large_zip() -> NamedTempFile {
    let tmp = NamedTempFile::new().unwrap();
    let writer = std::io::BufWriter::new(tmp.reopen().unwrap());
    let mut zip = zip::ZipWriter::new(writer);
    let opts = zip::write::SimpleFileOptions::default();
    for i in 0..10_000 {
        zip.start_file(format!("entry_{i:05}.txt"), opts).unwrap();
        zip.write_all(b"data").unwrap();
    }
    zip.finish().unwrap();
    tmp
}

fn bench_zip_list(c: &mut Criterion) {
    let tmp = build_large_zip();
    let archive_path = tmp.path().to_path_buf();

    let mut group = c.benchmark_group("archive_listing");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("zip_list_10k_entries", |b| {
        b.iter(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            rt.block_on(async {
                let fs = ZipFs::open(archive_path.clone()).unwrap();
                let root = VfsPath::parse("zip://archive/").unwrap();
                fs.list(&root, Sort::NameAsc).await.unwrap()
            })
        })
    });

    group.finish();
}

criterion_group!(benches, bench_zip_list);
criterion_main!(benches);
