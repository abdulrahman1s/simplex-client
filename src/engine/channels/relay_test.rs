//! TestChatRelay: a relay's address read, joined with a random challenge, and the relay's answer checked to be that challenge signed by the address's key; the test connection goes either way (Commands.hs `APITestChatRelay`; Subscriber.hs `RelayTest` CONF branch; Protocol.hs `XGrpRelayTest`).

use crate::agent::connection::LinkLookup;
use crate::agent::links::ConnectionLink;
use crate::agent::short_link::{UserLinkData, decode_short_link_data};
use crate::chat::ChatEvent;
use crate::crypto::random_vec;
use crate::engine::agent_port::{AgentPort, ConnId};
use crate::engine::error::Result;
use crate::engine::groups::group_conn_info;
use crate::engine::machine::Engine;
use crate::engine::records::{ConnKind, ConnRecord, Lookup, RelayTestRecord};
use crate::types::Event;
use crate::types::model::{RelayTestFailure, RelayTestResult, RelayTestStep as Step};

/// `timeout 40000000`: how long the relay has to answer.
const WAIT_MS: i64 = 40_000;

/// `RelayAddressLinkData` with its `RelayProfile`.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RelayAddress {
    relay_profile: RelayProfile,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RelayProfile {
    display_name: String,
}

pub(super) fn failed(
    address: &str,
    name: Option<String>,
    step: Step,
    error: &str,
) -> RelayTestResult {
    RelayTestResult {
        address: address.to_owned(),
        name,
        failure: Some(RelayTestFailure {
            step,
            error: error.to_owned(),
        }),
    }
}

impl<A: AgentPort> Engine<A> {
    pub(crate) fn test_chat_relay(&mut self, address: &str) -> Result<()> {
        let address = address.trim().to_owned();
        if let Err(e) = self.look_up(
            &address,
            Lookup::RelayTest {
                address: address.clone(),
            },
        ) {
            self.emit(Event::RelayTest(failed(
                &address,
                None,
                Step::GetLink,
                &e.detail,
            )));
        }
        Ok(())
    }

    /// `RTSGetLink`: the address's link could not be read.
    pub(super) fn relay_test_link_failed(&mut self, address: &str, detail: &str) -> Result<()> {
        self.emit(Event::RelayTest(failed(
            address,
            None,
            Step::GetLink,
            detail,
        )));
        Ok(())
    }

    pub(super) fn on_relay_test_link(&mut self, address: &str, data: LinkLookup) -> Result<()> {
        let relay: Option<RelayAddress> = decode_short_link_data(&UserLinkData(data.user_data))
            .ok()
            .flatten();
        let Some(relay) = relay else {
            let r = failed(
                address,
                None,
                Step::DecodeLink,
                "no relay address link data",
            );
            self.emit(Event::RelayTest(r));
            return Ok(());
        };
        let name = relay.relay_profile.display_name;
        let challenge = random_vec(32).map_err(|_| {
            crate::engine::error::fail(crate::engine::error::Code::RequestFailed, "no randomness")
        })?;
        let info = group_conn_info(ChatEvent::GrpRelayTest {
            challenge: challenge.clone(),
            signature: None,
        })?;
        let link = ConnectionLink::Full(data.conn_req);
        let conn = match self.agent.join(&mut self.store, &link, info, false) {
            Ok(c) => c,
            Err(e) => {
                self.emit(Event::RelayTest(failed(
                    address,
                    Some(name),
                    Step::Connect,
                    e.detail(),
                )));
                return Ok(());
            }
        };
        let test = RelayTestRecord {
            address: address.to_owned(),
            name,
            challenge,
            root_key: data.root_key.to_der(),
            deadline: self.clock.wall_ms() + WAIT_MS,
        };
        self.save_conn(ConnRecord {
            conn,
            kind: ConnKind::RelayTest(test),
        })?;
        Ok(())
    }

    pub(super) fn relay_test_of(&self, conn: &ConnId) -> Option<RelayTestRecord> {
        self.state.conns.get(conn)?.kind.relay_test().cloned()
    }
}
