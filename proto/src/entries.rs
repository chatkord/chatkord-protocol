//! How many values a frame decodes into before it is decoded: each message and each entry of a repeated field is an
//! allocation of its own, which an empty entry of two bytes on the wire makes as large as its whole type. The shape of
//! every message is read from the schema itself (`schema`), so no table here restates which fields are messages, nor
//! the most entries a list may hold.

use crate::schema::{SCHEMA, Schema, Shape};

/// The deepest a frame's messages nest before it is refused, as prost's own decoder refuses deeper.
const DEEPEST: usize = 100;

/// Why a frame's values were not counted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Uncounted {
    /// It is not a `WireMessage` the decoder would read, or it nests deeper than the decoder reads.
    Unreadable,
    /// A list holds more entries than the schema's `most` for it.
    PastMost,
}

/// The values `frame` decodes into, each message and each entry of a repeated field counted once.
pub(crate) fn entries(frame: &[u8]) -> Result<usize, Uncounted> {
    let schema = &*SCHEMA;
    walk(schema, frame, schema.wire, 0)
}

fn walk(schema: &Schema, mut bytes: &[u8], message: usize, depth: usize) -> Result<usize, Uncounted> {
    use Uncounted::Unreadable;
    if depth > DEEPEST {
        return Err(Unreadable);
    }
    let fields = &schema.messages.get(message).ok_or(Unreadable)?.fields;
    let mut count = 0usize;
    // The entries so far of each list here that states a most; empty, and so unallocated, for most messages.
    let mut listed: Vec<(u32, usize)> = Vec::new();
    while !bytes.is_empty() {
        let key = prost::decode_length_delimiter(&mut bytes).map_err(|_| Unreadable)?;
        let (number, wire) = (u32::try_from(key >> 3).map_err(|_| Unreadable)?, key & 0b111);
        match wire {
            0 => {
                prost::decode_length_delimiter(&mut bytes).map_err(|_| Unreadable)?;
            }
            1 => bytes = bytes.get(8..).ok_or(Unreadable)?,
            5 => bytes = bytes.get(4..).ok_or(Unreadable)?,
            2 => {
                let length = prost::decode_length_delimiter(&mut bytes).map_err(|_| Unreadable)?;
                let value = bytes.get(..length).ok_or(Unreadable)?;
                bytes = bytes.get(length..).ok_or(Unreadable)?;
                let field = fields.get(&number);
                if let Some(most) = field.and_then(|field| field.most) {
                    let at = match listed.iter().position(|(listed, _)| *listed == number) {
                        Some(at) => at,
                        None => {
                            listed.push((number, 0));
                            listed.len() - 1
                        }
                    };
                    let (_, held) = listed.get_mut(at).ok_or(Unreadable)?;
                    *held += 1;
                    if *held > most {
                        return Err(Uncounted::PastMost);
                    }
                }
                let held = field.and_then(|field| match field.shape {
                    Shape::Message(held) => held,
                    _ => None,
                });
                if field.is_some_and(|field| field.repeated || matches!(field.shape, Shape::Message(_))) {
                    count = count.checked_add(1).ok_or(Unreadable)?;
                }
                if let Some(held) = held {
                    count = count.checked_add(walk(schema, value, held, depth + 1)?).ok_or(Unreadable)?;
                }
            }
            // A group, which this schema has none of, or no wire type at all: the decoder refuses it too.
            _ => return Err(Unreadable),
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chatkord_v1::wire_message::Body;
    use crate::chatkord_v1::{
        EditMessage, Event, MemberSummary, Message, SendMessage, Span, VoiceOccupancy, WireMessage,
    };
    use crate::rules::MAX_SPANS;
    use prost::Message as _;

    fn frame(body: Body) -> Vec<u8> {
        WireMessage { seq: 1, ack_of: None, body: Some(body) }.encode_to_vec()
    }

    #[test]
    fn each_message_and_repeated_entry_is_counted_and_a_string_inside_one_is_not() {
        let person = MemberSummary { user_id: "ada".into(), display_name: "Ada".into(), ..MemberSummary::default() };
        let room = VoiceOccupancy { channel_id: "c1".into(), people: vec![person.clone(), person] };
        let event = Event { kind: Some(crate::chatkord_v1::event::Kind::VoiceOccupancy(room)), ..Event::default() };
        // The event, the room and its two people.
        assert_eq!(entries(&frame(Body::Event(event))), Ok(4));
    }

    #[test]
    fn a_frame_the_decoder_refuses_is_counted_as_none() {
        assert_eq!(entries(&[0x0b]), Err(Uncounted::Unreadable), "a group");
        assert_eq!(entries(&[0x12, 0x05, 0x00]), Err(Uncounted::Unreadable), "a length past the end");
    }

    #[test]
    fn a_list_past_its_most_is_refused_before_it_is_decoded_and_one_at_it_is_counted() {
        let send = |spans: usize| {
            let spans = vec![Span::default(); spans];
            frame(Body::SendMessage(SendMessage { spans, ..SendMessage::default() }))
        };
        // The send and each of its spans.
        assert_eq!(entries(&send(MAX_SPANS)), Ok(1 + MAX_SPANS));
        assert_eq!(entries(&send(MAX_SPANS + 1)), Err(Uncounted::PastMost));
        let edit = EditMessage { spans: vec![Span::default(); MAX_SPANS + 1], ..EditMessage::default() };
        assert_eq!(entries(&frame(Body::EditMessage(edit))), Err(Uncounted::PastMost));
        let told = Message { spans: vec![Span::default(); MAX_SPANS + 1], ..Message::default() };
        assert_eq!(entries(&frame(Body::Message(told))), Ok(2 + MAX_SPANS), "a list stating no most is only counted");
    }
}
