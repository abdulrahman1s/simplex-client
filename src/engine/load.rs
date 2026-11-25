//! Reading the records back at start, and writing them through the cache.

use crate::types::model::ChatRef;
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::agent_port::AgentPort;
use super::machine::Engine;
use super::records::{
    ConnRecord, FriendRecord, GroupRecord, ItemRecord, MemberRecord, NS, SettingsRecord,
    UserRecord, decode, encode, keys,
};
use super::state::State;
use crate::store::{Store, StoreError};

fn get<T: DeserializeOwned>(store: &mut Store, key: &[u8]) -> Result<Option<T>, StoreError> {
    store.get(NS, key)?.map(|b| decode(&b)).transpose()
}

fn all<T: DeserializeOwned>(store: &mut Store, prefix: &[u8]) -> Result<Vec<T>, StoreError> {
    let mut out = Vec::new();
    for key in store.keys_with_prefix(NS, prefix)? {
        out.extend(get(store, &key)?);
    }
    Ok(out)
}

fn last_item(
    store: &mut Store,
    state: &mut State,
    chat: ChatRef,
    read_up_to: i64,
) -> Result<(), StoreError> {
    if let Some((id, bytes)) = store.page(&keys::chat(chat), None, 1)?.pop() {
        let item: ItemRecord = decode(&bytes)?;
        state.last.insert(chat, item.to_wire(id, chat, read_up_to));
    }
    Ok(())
}

/// The state as the store has it. `None` for the user on a first start.
pub fn load(store: &mut Store) -> Result<(State, Option<UserRecord>), StoreError> {
    let mut state = State {
        settings: get::<SettingsRecord>(store, keys::SETTINGS)?.unwrap_or_default(),
        ..State::default()
    };
    for c in all::<ConnRecord>(store, keys::CONN_PREFIX)? {
        state.conns.insert(c.conn, c);
    }
    for f in all::<FriendRecord>(store, keys::FRIEND_PREFIX)? {
        last_item(store, &mut state, ChatRef::Direct(f.id), f.read_up_to)?;
        state.friends.insert(f.id, f);
    }
    for g in all::<GroupRecord>(store, keys::GROUP_PREFIX)? {
        last_item(store, &mut state, ChatRef::Group(g.id), g.read_up_to)?;
        state.groups.insert(g.id, g);
    }
    for m in all::<MemberRecord>(store, keys::MEMBER_PREFIX)? {
        state.members.insert((m.group, m.id), m);
    }
    state.timed.extend(super::timed::index::load_due(store)?);
    state
        .timed_unread
        .extend(super::timed::index::load_unread(store)?);
    let user = get::<UserRecord>(store, keys::USER)?;
    Ok((state, user))
}

impl<A: AgentPort> Engine<A> {
    pub(crate) fn put<T: Serialize>(&mut self, key: &[u8], record: &T) -> Result<(), StoreError> {
        let bytes = encode(record)?;
        self.store.put(NS, key, &bytes)
    }

    pub(crate) fn save_settings(&mut self, s: SettingsRecord) -> Result<(), StoreError> {
        self.put(keys::SETTINGS, &s)?;
        self.state.settings = s;
        Ok(())
    }

    /// The user's profile as the page sees it, with the settings kept beside it.
    pub(crate) fn profile_wire(&self) -> crate::types::model::ChatProfile {
        let mut p = self.state.user.to_wire();
        p.item_ttl = self.state.settings.item_ttl;
        if let Some(a) = &self.state.address.record {
            p.auto_accept = a.auto_accept;
            p.auto_reply.clone_from(&a.auto_reply);
        }
        p
    }

    pub(crate) fn save_friend(&mut self, f: FriendRecord) -> Result<(), StoreError> {
        self.put(&keys::friend(f.id), &f)?;
        self.state.changed.friends.insert(f.id);
        self.state.friends.insert(f.id, f);
        Ok(())
    }

    pub(crate) fn save_group(&mut self, g: GroupRecord) -> Result<(), StoreError> {
        self.put(&keys::group(g.id), &g)?;
        self.state.changed.groups.insert(g.id);
        self.state.groups.insert(g.id, g);
        Ok(())
    }

    pub(crate) fn save_member(&mut self, m: MemberRecord) -> Result<(), StoreError> {
        self.put(&keys::member(m.group, m.id), &m)?;
        self.state.changed.groups.insert(m.group);
        self.state.members.insert((m.group, m.id), m);
        Ok(())
    }

    pub(crate) fn save_conn(&mut self, c: ConnRecord) -> Result<(), StoreError> {
        self.put(&keys::conn(&c.conn), &c)?;
        self.state.conns.insert(c.conn, c);
        Ok(())
    }

    pub(crate) fn save_user(&mut self, u: UserRecord) -> Result<(), StoreError> {
        self.put(keys::USER, &u)?;
        // Every friend shows the user's preferences (their timer and features).
        self.state.changed.all = true;
        self.state.user = u;
        Ok(())
    }

    /// A small index row: an item number under `key`.
    pub(crate) fn put_index(&mut self, key: &[u8], item: i64) -> Result<(), StoreError> {
        self.store.put(NS, key, &item.to_le_bytes())
    }

    pub(crate) fn get_index(&mut self, key: &[u8]) -> Result<Option<i64>, StoreError> {
        let Some(b) = self.store.get(NS, key)? else {
            return Ok(None);
        };
        let b: [u8; 8] = b
            .try_into()
            .map_err(|_| StoreError::Corrupt("bad index row".into()))?;
        Ok(Some(i64::from_le_bytes(b)))
    }
}
