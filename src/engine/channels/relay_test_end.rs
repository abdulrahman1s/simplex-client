//! A relay test's end: the relay's signed answer checked, a failed join or the 40 seconds passed, then the test connection deleted and the result told (Subscriber.hs `RelayTest` CONF branch; Commands.hs `APITestChatRelay`).

use crate::chat::{ChatEvent, ChatMessage};
use crate::crypto::{Alg, PublicKey, Signature};
use crate::engine::agent_port::{AgentPort, ConnId};
use crate::engine::error::Result;
use crate::engine::groups::parse_conn_info;
use crate::engine::machine::Engine;
use crate::engine::records::RelayTestRecord;
use crate::types::Event;
use crate::types::model::{RelayTestResult, RelayTestStep as Step};

use super::relay_test::failed;

impl<A: AgentPort> Engine<A> {
    /// The relay's CONF: its `x.grp.relay.test` with the challenge's signature.
    pub(crate) fn on_relay_test_conf(&mut self, conn: &ConnId, info: &[u8]) -> Option<Result<()>> {
        let t = self.relay_test_of(conn)?;
        let failure = match parse_conn_info(info).map(|m: ChatMessage| m.event) {
            Ok(ChatEvent::GrpRelayTest {
                signature: Some(sig),
                ..
            }) => verify(&t, &sig),
            Ok(ChatEvent::GrpRelayTest {
                signature: None, ..
            }) => Some((Step::Verify, "no signature in response".to_owned())),
            Ok(_) => Some((Step::WaitResponse, "unexpected message type".to_owned())),
            Err(e) => Some((Step::WaitResponse, e.detail)),
        };
        Some(self.finish_relay_test(conn, &t, failure))
    }

    /// The agent gave up on the join.
    pub(crate) fn on_relay_test_error(
        &mut self,
        conn: &ConnId,
        detail: &str,
    ) -> Option<Result<()>> {
        let t = self.relay_test_of(conn)?;
        Some(self.finish_relay_test(conn, &t, Some((Step::Connect, detail.to_owned()))))
    }

    #[must_use]
    pub(crate) fn next_relay_test_wake(&self) -> Option<i64> {
        (self.state.conns.values())
            .filter_map(|c| c.kind.relay_test().map(|t| t.deadline))
            .min()
    }

    pub(crate) fn expire_relay_tests(&mut self, now: i64) {
        let due: Vec<(ConnId, RelayTestRecord)> = (self.state.conns.iter())
            .filter_map(|(id, c)| c.kind.relay_test().map(|t| (*id, t.clone())))
            .filter(|(_, t)| t.deadline <= now)
            .collect();
        for (conn, t) in due {
            if let Err(e) =
                self.finish_relay_test(&conn, &t, Some((Step::WaitResponse, "timeout".to_owned())))
            {
                self.log
                    .line(format_args!("a relay test did not end: {}", e.detail));
            }
        }
    }

    /// The test connection goes, as 7.1.4 deletes it, and the result is told.
    fn finish_relay_test(
        &mut self,
        conn: &ConnId,
        t: &RelayTestRecord,
        failure: Option<(Step, String)>,
    ) -> Result<()> {
        self.forget_conn(conn)?;
        let result = match failure {
            Some((step, error)) => failed(&t.address, Some(t.name.clone()), step, &error),
            None => RelayTestResult {
                address: t.address.clone(),
                name: Some(t.name.clone()),
                failure: None,
            },
        };
        self.emit(Event::RelayTest(result));
        Ok(())
    }
}

/// `C.verify' rootKey sig challenge`: the raw challenge, no binding.
fn verify(t: &RelayTestRecord, sig: &[u8]) -> Option<(Step, String)> {
    let Ok(key @ PublicKey::Ed25519(_)) = PublicKey::from_der(&t.root_key) else {
        return Some((Step::Verify, "invalid signature".to_owned()));
    };
    match Signature::from_raw(Alg::Ed25519, sig) {
        Ok(s) if key.verify(&s, &t.challenge) => None,
        Ok(_) => Some((Step::Verify, "invalid signature".to_owned())),
        Err(e) => Some((Step::Verify, format!("signature decoding failed: {e}"))),
    }
}
