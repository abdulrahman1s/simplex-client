//! The XFTP file transfer client: protocol 1 to 4 over HTTP/2 over our TLS, descriptions and file encryption.
//! Ported from simplexmq FileTransfer/*.hs and nook src/sx/xftp (AGPL-3.0-only).

mod address;
mod chunks;
#[cfg(feature = "native")]
mod client;
mod commands;
#[cfg(feature = "native")]
mod connect;
#[cfg(feature = "native")]
mod delete;
pub mod description;
#[cfg(feature = "native")]
mod download;
#[cfg(feature = "native")]
mod download_fetch;
mod error;
mod error_type;
mod file_decrypt;
#[cfg(feature = "native")]
mod file_digest;
mod file_encrypt;
mod file_header;
mod handshake;
mod handshake_msgs;
#[cfg(feature = "native")]
mod ops;
#[cfg(feature = "native")]
mod ops_download;
#[cfg(feature = "native")]
mod pool;
mod progress;
mod responses;
mod safe_name;
mod servers;
mod signed_key;
mod transmission;
#[cfg(feature = "native")]
mod upload;
#[cfg(feature = "native")]
mod upload_chunk;
#[cfg(feature = "native")]
mod upload_descr;
#[cfg(feature = "native")]
mod upload_prepare;
mod version;

pub use address::XftpServer;
pub use chunks::{
    CHUNK_SIZE_0, CHUNK_SIZE_1, CHUNK_SIZE_2, CHUNK_SIZE_3, ChunkSpec, FILE_SIZE_LEN, GB, KB,
    MAX_FILE_SIZE, MAX_FILE_SIZE_HARD, MB, SERVER_CHUNK_SIZES, encrypted_size, prepare_chunk_sizes,
    prepare_chunk_specs, single_chunk_size,
};
#[cfg(feature = "native")]
pub use client::{ClientConfig, Ctl, XftpClient};
pub use commands::{Command, FileInfo};
#[cfg(feature = "native")]
pub use connect::{TcpXftpClient, connect};
#[cfg(feature = "native")]
pub use delete::delete_file;
#[cfg(feature = "native")]
pub use download::{DownloadParams, Downloaded, download};
pub use error::{HandshakeError, XftpError};
pub use error_type::{CommandError, XftpErrorType};
#[cfg(feature = "native")]
pub use file_decrypt::decrypt_chunks;
pub use file_decrypt::decrypt_bytes;
#[cfg(feature = "native")]
pub use file_encrypt::{EncryptParams, encrypt_file};
pub use file_encrypt::encrypt_bytes;
pub use file_header::FileHeader;
pub use handshake::{Negotiated, Patience, TlsFacts, client_handshake};
pub use handshake_msgs::{ServerHandshake, encode_client_handshake};
#[cfg(feature = "native")]
pub use ops::NewChunk;
pub use progress::{CancelFlag, ProgressFn};
pub use responses::Response;
#[cfg(feature = "native")]
pub use safe_name::claim_unique;
pub use safe_name::safe_file_name;
pub use servers::{FLUX, SIMPLEX, preset_servers};
pub use transmission::{decode_transmission, encode_transmission};
#[cfg(feature = "native")]
pub use upload::{UploadParams, Uploaded, upload};
pub use version::{
    ALPN, AUTH_TAG_SIZE, BLOCK_SIZE, BLOCKED_FILES, DEFAULT_PORT, FILE_STORAGE_TIME, MAX_VERSION,
    MIN_VERSION,
};
