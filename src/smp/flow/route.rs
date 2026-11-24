//! What one received transmission means for the owner: an event, an answer, or both (Client.hs `processMsg`).
use crate::encoding::DecodeError;
use crate::smp::error::SmpError;
use crate::smp::events::SmpEventKind;
use crate::smp::responses::Response;

use super::request::Answer;

pub struct Routed {
    /// Comes before the answer, so a MSG that answers SUB, ACK or GET reaches the owner first as a message.
    pub event: Option<SmpEventKind>,
    /// Present when a command was waiting on the corrId.
    pub answer: Option<Answer>,
}

/// `waited` is the entity of the command waiting on this transmission's corrId, if one was.
pub fn route(
    decoded: Result<Response, DecodeError>,
    entity_id: Vec<u8>,
    waited: Option<&[u8]>,
) -> Routed {
    let Some(waited) = waited else {
        // An unreadable push has no one to be told to.
        let event = decoded.ok().map(|r| push_event(entity_id, r));
        return Routed {
            event,
            answer: None,
        };
    };
    let event = match &decoded {
        Ok(r @ Response::Msg { .. }) => Some(push_event(entity_id.clone(), r.clone())),
        _ => None,
    };
    let answer = match decoded {
        Ok(Response::Err(e)) => Err(SmpError::Server(e)),
        Ok(_) if waited != entity_id => Err(SmpError::Unexpected("response for another queue")),
        Ok(r) => Ok(r),
        Err(e) => Err(e.into()),
    };
    Routed {
        event,
        answer: Some(answer),
    }
}

fn push_event(entity_id: Vec<u8>, r: Response) -> SmpEventKind {
    match r {
        Response::Msg { msg_id, body } => SmpEventKind::Msg {
            entity_id,
            msg_id,
            body,
        },
        Response::End => SmpEventKind::End { entity_id },
        Response::Deld => SmpEventKind::Deld { entity_id },
        other @ (Response::Ids(_)
        | Response::Lnk { .. }
        | Response::Ok
        | Response::Sok(_)
        | Response::Nid { .. }
        | Response::Info(_)
        | Response::Pkey { .. }
        | Response::Pres { .. }
        | Response::Err(_)
        | Response::Pong) => SmpEventKind::Unexpected {
            entity_id,
            response: other,
        },
    }
}
