//! JSON name and params to `ChatEvent` (Protocol.hs `appJsonToCM`).
use super::kind::EventKind as K;
use super::types::ChatEvent as E;
use crate::chat::error::{Result, parse_err};
use crate::chat::json::{opt, req};
use crate::chat::limits::MAX_FWD_DEPTH;
use crate::chat::message::ChatMessage;
mod group;
mod misc;
mod msg;
mod relay;

use group::{decode_group, decode_join, decode_member};
use misc::{decode_call, decode_probe};
use msg::{decode_conn, decode_msg};
use serde_json::{Map, Value};

pub(super) type Obj = Map<String, Value>;

/// `depth` counts the `x.grp.msg.forward` envelopes already entered.
pub(crate) fn decode(name: &str, o: &Obj, depth: usize) -> Result<E> {
    if !name.starts_with('x') {
        return parse_err("bad event tag");
    }
    let Some(kind) = K::from_name(name) else {
        return Ok(unknown(name, o));
    };
    match kind {
        K::MsgNew | K::MsgFileDescr | K::MsgUpdate | K::MsgDel | K::MsgDeleted | K::MsgReact => {
            decode_msg(kind, o)
        }
        K::File
        | K::FileAcpt
        | K::FileAcptInv
        | K::FileCancel
        | K::Info
        | K::Contact
        | K::Member
        | K::DirectDel
        | K::Ok => decode_conn(kind, o),
        K::GrpInv
        | K::GrpAcpt
        | K::GrpLinkInv
        | K::GrpLinkReject
        | K::GrpLinkMem
        | K::GrpLinkAcpt => decode_join(kind, o),
        K::GrpMemNew
        | K::GrpMemIntro
        | K::GrpMemInv
        | K::GrpMemFwd
        | K::GrpMemInfo
        | K::GrpMemRole
        | K::GrpMemRestrict => decode_member(kind, o),
        K::GrpMemCon
        | K::GrpMemConAll
        | K::GrpMemDel
        | K::GrpLeave
        | K::GrpDel
        | K::GrpInfo
        | K::GrpPrefs
        | K::GrpDirectInv => decode_group(kind, o),
        K::GrpMsgForward => decode_forward(o, depth),
        K::InfoProbe | K::InfoProbeCheck | K::InfoProbeOk => decode_probe(kind, o),
        K::CallInv | K::CallOffer | K::CallAnswer | K::CallExtra | K::CallEnd => {
            decode_call(kind, o)
        }
        K::GrpRelayInv => relay::inv(o),
        K::GrpRelayAcpt => relay::acpt(o),
        K::GrpRelayTest => relay::test(o),
        K::GrpRelayNew => relay::new(o),
        K::GrpRelayReject => relay::reject(o),
        K::GrpRelayCap => relay::cap(o),
        K::GrpRoster => relay::roster(o),
        K::GrpRosterAck => relay::roster_ack(o),
        K::GrpRosterRequest => relay::roster_request(o),
    }
}

fn unknown(name: &str, o: &Obj) -> E {
    E::Unknown {
        event: name.to_owned(),
        params: o.clone(),
    }
}

fn decode_forward(o: &Obj, depth: usize) -> Result<E> {
    use crate::chat::forward::{FwdSender, GrpMsgForward};
    if depth >= MAX_FWD_DEPTH {
        return parse_err("forward depth exceeds limit");
    }
    let sender = match opt(o, "memberId")? {
        Some(member_id) => FwdSender::Member {
            member_id,
            member_name: opt(o, "memberName")?.unwrap_or_default(),
        },
        None => FwdSender::Channel,
    };
    let msg: Value = req(o, "msg")?;
    let broker_ts: String = req(o, "msgTs")?;
    // Aeson's `UTCTime` refuses a date it cannot read; so does this.
    if crate::chat::iso::iso_to_secs(&broker_ts).is_none() {
        return parse_err("bad msgTs");
    }
    let fwd = GrpMsgForward { sender, broker_ts };
    Ok(E::GrpMsgForward {
        fwd,
        msg: Box::new(ChatMessage::from_value(&msg, depth + 1)?),
    })
}
