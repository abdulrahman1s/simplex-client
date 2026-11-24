//! The blocking download refuses a redirect and a file larger than allowed before it dials anyone.
use std::sync::atomic::AtomicBool;

use crate::xftp::description::{ChunkReplica, FileChunk, FileDescription, Party, Redirect};
use crate::xftp::{DownloadParams, XftpError, XftpServer, download};

/// A description on a server nobody listens on: any dial would fail with a network error.
fn unreachable(size_chunks: u32) -> FileDescription {
    let server: XftpServer = "xftp://AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=@127.0.0.1:1"
        .parse()
        .unwrap();
    let chunks = (1..=size_chunks)
        .map(|chunk_no| FileChunk {
            chunk_no,
            chunk_size: 65536,
            digest: vec![7; 32],
            replicas: vec![ChunkReplica {
                server: server.clone(),
                replica_id: vec![1; 24],
                replica_key: vec![2; 48],
            }],
        })
        .collect();
    FileDescription {
        party: Party::Recipient,
        size: 65536 * u64::from(size_chunks),
        digest: vec![3; 64],
        key: vec![4; 32],
        nonce: vec![5; 24],
        chunk_size: 65536,
        chunks,
        redirect: None,
    }
}

fn try_download(fd: &FileDescription, max_size: u64) -> XftpError {
    let dir = std::env::temp_dir().join("simplex-native-limits");
    let params = DownloadParams {
        description: fd,
        work_dir: &dir,
        dest_dir: &dir,
        max_size,
    };
    download(&params, &AtomicBool::new(false), &mut |_, _| {}).unwrap_err()
}

#[test]
fn a_native_download_refuses_a_redirect_description() {
    let mut fd = unreachable(1);
    fd.redirect = Some(Redirect {
        size: 65536,
        digest: vec![9; 64],
    });
    let e = try_download(&fd, u64::MAX);
    assert!(
        matches!(&e, XftpError::Description(d) if d.contains("redirect")),
        "{e:?}"
    );
}

#[test]
fn a_native_download_refuses_a_file_larger_than_offered() {
    let e = try_download(&unreachable(3), 2 * 65536);
    assert!(
        matches!(&e, XftpError::File(d) if d.contains("offered")),
        "{e:?}"
    );
}
