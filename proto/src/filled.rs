//! Every op the schema holds with every field set, its secrets to a text a test can look for: what a test of a router
//! or a redaction walks, so a field the schema grows is walked with no list to keep.

use crate::chatkord_v1::WireMessage;
use crate::chatkord_v1::wire_message::Body;
use crate::schema::{SCHEMA, Schema, Shape};
use prost::Message as _;
use prost::encoding::{WireType, encode_key, encode_varint};
use std::collections::BTreeSet;

/// Deep enough for every message an op holds; a message that holds itself stops here.
const DEEPEST: usize = 6;
/// More than any oneof of the schema has members, so each member is chosen once.
const PICKS: usize = 8;

/// One op, filled.
#[derive(Debug)]
pub struct Filled {
    /// The op, every field set: a secret to the given text and its field's name, any other text to its name.
    pub op: Body,
    /// The op's name in the schema.
    pub name: String,
    /// The secret fields it set, as `Message.field`.
    pub secrets: BTreeSet<String>,
    /// The plain texts it set, which a print shows.
    pub plain: Vec<String>,
}

/// Every op, once for each member of each oneof it holds, its secrets set to `secret` and their field's name.
#[expect(
    clippy::expect_used,
    reason = "a test's fixture; a frame built from the schema decodes or the schema is wrong"
)]
pub fn every_op_filled(secret: &str) -> Vec<Filled> {
    let schema = &*SCHEMA;
    let wire = &schema.messages[schema.wire];
    let mut all = Vec::new();
    for (&number, op) in &wire.fields {
        let (Shape::Message(Some(held)), Some(_)) = (op.shape, op.oneof) else { continue };
        for pick in 0..PICKS {
            let mut into = Walked::default();
            let body = filled(schema, secret, held, pick, 1, &mut into);
            let mut frame = Vec::new();
            encode_key(number, WireType::LengthDelimited, &mut frame);
            encode_varint(body.len() as u64, &mut frame);
            frame.extend_from_slice(&body);
            let message = WireMessage::decode(frame.as_slice()).expect("a frame of the schema decodes");
            let op_body = message.body.expect("an op");
            all.push(Filled { op: op_body, name: op.name.clone(), secrets: into.secrets, plain: into.plain });
        }
    }
    all
}

/// Every field the schema marks secret, as `Message.field`.
pub fn marked_secrets() -> BTreeSet<String> {
    SCHEMA
        .messages
        .iter()
        .flat_map(|message| {
            message.fields.values().filter(|field| field.secret).map(|field| format!("{}.{}", message.name, field.name))
        })
        .collect()
}

/// What a frame was filled with: the secrets it set and the plain texts it set, by their field.
#[derive(Default)]
struct Walked {
    secrets: BTreeSet<String>,
    plain: Vec<String>,
}

/// Message `at` encoded with every field set, choosing the `pick`-th member of each oneof: a secret to `SECRET`
/// and its name, a plain text to its own name.
fn filled(schema: &Schema, secret: &str, at: usize, pick: usize, depth: usize, into: &mut Walked) -> Vec<u8> {
    let mut bytes = Vec::new();
    let Some(message) = schema.messages.get(at) else { return bytes };
    let mut numbers: Vec<&u32> = message.fields.keys().collect();
    numbers.sort();
    for &number in numbers {
        let field = &message.fields[&number];
        if let Some(oneof) = field.oneof {
            let mut members: Vec<u32> =
                message.fields.iter().filter(|(_, other)| other.oneof == Some(oneof)).map(|(n, _)| *n).collect();
            members.sort_unstable();
            if members[pick % members.len()] != number {
                continue;
            }
        }
        let name = format!("{}.{}", message.name, field.name);
        match field.shape {
            Shape::Varint => {
                encode_key(number, WireType::Varint, &mut bytes);
                encode_varint(1, &mut bytes);
            }
            Shape::Fixed64 => {
                encode_key(number, WireType::SixtyFourBit, &mut bytes);
                bytes.extend_from_slice(&[0; 8]);
            }
            Shape::Fixed32 => {
                encode_key(number, WireType::ThirtyTwoBit, &mut bytes);
                bytes.extend_from_slice(&[0; 4]);
            }
            Shape::Text | Shape::Bytes => {
                let value = if field.secret {
                    into.secrets.insert(name.clone());
                    format!("{secret}-{name}")
                } else {
                    if field.shape == Shape::Text {
                        into.plain.push(name.clone());
                    }
                    name
                };
                encode_key(number, WireType::LengthDelimited, &mut bytes);
                encode_varint(value.len() as u64, &mut bytes);
                bytes.extend_from_slice(value.as_bytes());
            }
            Shape::Message(Some(held)) if depth < DEEPEST => {
                let inner = filled(schema, secret, held, pick, depth + 1, into);
                encode_key(number, WireType::LengthDelimited, &mut bytes);
                encode_varint(inner.len() as u64, &mut bytes);
                bytes.extend_from_slice(&inner);
            }
            Shape::Message(_) => {}
        }
    }
    bytes
}
