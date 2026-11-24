//! Setting files up at the start: the data folder, the file servers, and the transfers a restart cut short.

use crate::types::{Config, has_file_servers};

use super::transfers::{chat_of, owner};
use crate::engine::agent_port::AgentPort;
use crate::engine::machine::Engine;
use crate::engine::records::file::{FileRecord, Way};
use crate::engine::records::{NS, decode, keys};
use crate::store::StoreError;
use crate::types::model::{ChatRef, FileState};
use crate::xftp::{XftpServer, preset_servers};

/// The file servers of a Config: its `xftp://` relays, the presets when it names no relay of
/// its own, and none when it names own relays only (`has_file_servers`).
fn servers_of(relays: &[String]) -> Vec<XftpServer> {
    let own: Vec<XftpServer> = relays
        .iter()
        .filter(|r| r.starts_with("xftp://"))
        .filter_map(|r| r.parse().ok())
        .collect();
    if own.is_empty() && has_file_servers(relays) {
        preset_servers()
    } else {
        own
    }
}

/// `(owner, item)` of a `file/{owner}/{item}` key (`transfers::owner`).
fn ids_of(key: &[u8]) -> Option<(i64, i64)> {
    let rest = std::str::from_utf8(key).ok()?.strip_prefix("file/")?;
    let (friend, item) = rest.split_once('/')?;
    Some((friend.parse().ok()?, item.parse().ok()?))
}

impl<A: AgentPort> Engine<A> {
    /// Resuming a half-done transfer is left out: a received file goes back to being offered
    /// (its description is kept), a sent one is marked failed.
    pub(crate) fn configure_files(&mut self, hello: &Config) -> Result<(), StoreError> {
        self.files.dir.clone_from(&hello.dir);
        self.files.servers = servers_of(&hello.relays);
        // Without threads and a folder (a browser), transfers stay `NoTransfers`.
        #[cfg(feature = "native")]
        self.open_files_folder();
        for key in self.store.keys_with_prefix(NS, keys::FILE_PREFIX)? {
            let (Some((friend, item)), Some(bytes)) = (ids_of(&key), self.store.get(NS, &key)?)
            else {
                continue;
            };
            let rec: FileRecord = decode(&bytes)?;
            if rec.active {
                self.end_cut_short(chat_of(friend), item, rec)?;
            }
        }
        Ok(())
    }

    #[cfg(feature = "native")]
    fn open_files_folder(&mut self) {
        self.files.transfers = Box::new(super::real::XftpTransfers::new());
        if let Err(e) = std::fs::create_dir_all(self.files.dir.join("files")) {
            self.log.line(format_args!("files folder not made: {e}"));
        }
        // Nothing resumes, so what a killed transfer left is only in the way.
        let _ = std::fs::remove_dir_all(self.files.dir.join("xftp-tmp")); // not there is fine
    }

    fn end_cut_short(
        &mut self,
        chat: ChatRef,
        item: i64,
        mut rec: FileRecord,
    ) -> Result<(), StoreError> {
        rec.active = false;
        rec.accepted = false;
        let state = match rec.way {
            Way::Sent => FileState::Failed,
            Way::Received => FileState::Offered,
        };
        self.put(&keys::file(owner(chat), item), &rec)?;
        if let Err(e) = self.show_file(chat, item, &rec, state) {
            self.log
                .line(format_args!("a cut-short transfer not shown: {}", e.detail));
        }
        Ok(())
    }
}

#[cfg(test)]
#[cfg(feature = "native")]
mod tests {
    use super::*;

    #[test]
    fn own_relays_without_an_own_file_server_get_no_preset_file_server() {
        let relay = "smp://hpq7_4gGJiilmz5Rf-CswuU5kZGkm_zOIooSw6yALRg=@relay.test".to_owned();
        let files = "xftp://hpq7_4gGJiilmz5Rf-CswuU5kZGkm_zOIooSw6yALRg=@files.test".to_owned();
        assert_eq!(servers_of(&[]), preset_servers());
        assert!(servers_of(std::slice::from_ref(&relay)).is_empty());
        assert_eq!(
            servers_of(&[relay, files.clone()]),
            [files.parse().unwrap()]
        );
    }
}
