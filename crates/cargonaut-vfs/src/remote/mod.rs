// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Remote VFS backends — read/write backends for SFTP and FTP servers.

pub mod ftp_fs;
pub mod sftp_fs;

pub use ftp_fs::{FtpFs, FtpOps};
pub use sftp_fs::{HostKeyEvent, SftpCredentials, SftpFs, SftpOps};
