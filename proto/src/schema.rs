//! The schema as the generated code carries it (`chatkord_v1::FILE_DESCRIPTOR_SET`, written from `wire/`), read once:
//! every message's fields by number, each field's shape, whether `wire/chatkord/v1/options.proto` marks it a secret
//! and the most entries it may hold. The descriptor is decoded by the few `descriptor.proto` fields read here, since prost's own descriptor
//! types drop the options a schema adds.

use crate::chatkord_v1::FILE_DESCRIPTOR_SET;
use prost::Message as _;
use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Clone, PartialEq, prost::Message)]
struct FileSet {
    #[prost(message, repeated, tag = "1")]
    file: Vec<File>,
}

#[derive(Clone, PartialEq, prost::Message)]
struct File {
    #[prost(string, optional, tag = "2")]
    package: Option<String>,
    #[prost(message, repeated, tag = "4")]
    message_type: Vec<DescriptorMessage>,
    #[cfg(feature = "codegen")]
    #[prost(message, repeated, tag = "5")]
    enum_type: Vec<DescriptorEnum>,
}

#[cfg(feature = "codegen")]
#[derive(Clone, PartialEq, prost::Message)]
struct DescriptorEnum {
    #[prost(string, optional, tag = "1")]
    name: Option<String>,
    #[prost(message, repeated, tag = "2")]
    value: Vec<EnumValue>,
}

#[cfg(feature = "codegen")]
#[derive(Clone, PartialEq, prost::Message)]
struct EnumValue {
    #[prost(string, optional, tag = "1")]
    name: Option<String>,
    #[prost(int32, optional, tag = "2")]
    number: Option<i32>,
}

#[derive(Clone, PartialEq, prost::Message)]
struct DescriptorMessage {
    #[prost(string, optional, tag = "1")]
    name: Option<String>,
    #[prost(message, repeated, tag = "2")]
    field: Vec<DescribedField>,
    #[prost(message, repeated, tag = "3")]
    nested_type: Vec<DescriptorMessage>,
    #[prost(message, optional, tag = "7")]
    options: Option<MessageOptions>,
}

#[derive(Clone, PartialEq, prost::Message)]
struct MessageOptions {
    // `options.proto`'s `sent_by`, a `Sender`.
    #[prost(int32, optional, tag = "50001")]
    sent_by: Option<i32>,
    // `options.proto`'s `reply`.
    #[prost(string, optional, tag = "50002")]
    reply: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
struct DescribedField {
    #[prost(string, optional, tag = "1")]
    name: Option<String>,
    #[prost(int32, optional, tag = "3")]
    number: Option<i32>,
    #[prost(int32, optional, tag = "4")]
    label: Option<i32>,
    #[prost(int32, optional, tag = "5")]
    r#type: Option<i32>,
    #[prost(string, optional, tag = "6")]
    type_name: Option<String>,
    #[prost(message, optional, tag = "8")]
    options: Option<Options>,
    #[prost(int32, optional, tag = "9")]
    oneof_index: Option<i32>,
    #[prost(bool, optional, tag = "17")]
    proto3_optional: Option<bool>,
}

#[derive(Clone, PartialEq, prost::Message)]
struct Options {
    // `options.proto`'s `secret`.
    #[prost(bool, optional, tag = "50000")]
    secret: Option<bool>,
    // `options.proto`'s `most`.
    #[prost(uint32, optional, tag = "50003")]
    most: Option<u32>,
}

/// What a field's values are on the wire, by `descriptor.proto`'s `Type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shape {
    /// A varint: an integer, a bool or an enum.
    Varint,
    /// Eight bytes.
    Fixed64,
    /// Four bytes.
    Fixed32,
    /// Text.
    Text,
    /// Bytes.
    Bytes,
    /// A message, by its index in `Schema::messages`; none for one outside the schema.
    Message(Option<usize>),
}

/// One field of a message.
#[derive(Clone, Debug)]
pub(crate) struct Field {
    /// Its name in the schema.
    #[cfg(any(test, feature = "codegen", feature = "testing"))]
    pub(crate) name: String,
    /// Whether it holds any number of values.
    pub(crate) repeated: bool,
    /// The most entries the schema lets it hold; none when it states none.
    pub(crate) most: Option<usize>,
    /// What each value is.
    pub(crate) shape: Shape,
    /// Whether the schema marks it a secret.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) secret: bool,
    /// The oneof it is a member of, by its index in the message; none for a plain field or a proto3 `optional`.
    #[cfg(any(test, feature = "codegen", feature = "testing"))]
    pub(crate) oneof: Option<i32>,
}

/// One message of the schema.
pub(crate) struct Described {
    /// Its full name, `.chatkord.v1.Login`.
    #[cfg(any(test, feature = "codegen", feature = "testing"))]
    pub(crate) name: String,
    /// Who sends it, as `options.proto`'s `Sender` numbers them; none when unsaid.
    #[cfg(feature = "codegen")]
    pub(crate) sent_by: Option<i32>,
    /// The name of the message that answers it, when it is an ask.
    #[cfg(feature = "codegen")]
    pub(crate) reply: Option<String>,
    /// Its fields by number.
    pub(crate) fields: HashMap<u32, Field>,
}

/// Every message of the schema, by index.
pub(crate) struct Schema {
    pub(crate) messages: Vec<Described>,
    /// `WireMessage`'s index.
    pub(crate) wire: usize,
    /// Each top-level enum's name and its values' names, in the order the schema states them, with `UNSPECIFIED`'s
    /// zero among them.
    #[cfg(feature = "codegen")]
    pub(crate) enums: Vec<(String, Vec<String>)>,
}

/// The schema, read at its first use; one it cannot read has no messages, so every frame is refused.
pub(crate) static SCHEMA: LazyLock<Schema> = LazyLock::new(Schema::read);

impl Schema {
    fn read() -> Self {
        let files = FileSet::decode(FILE_DESCRIPTOR_SET).map(|set| set.file).unwrap_or_default();
        let mut named = Vec::new();
        for file in &files {
            let package = file.package.as_deref().unwrap_or_default();
            for message in &file.message_type {
                gather(&format!(".{package}"), message, &mut named);
            }
        }
        let index: HashMap<String, usize> =
            named.iter().enumerate().map(|(at, (name, _))| (name.clone(), at)).collect();
        let messages = named
            .iter()
            .map(|(_name, message)| Described {
                #[cfg(any(test, feature = "codegen", feature = "testing"))]
                name: _name.clone(),
                #[cfg(feature = "codegen")]
                sent_by: message.options.as_ref().and_then(|options| options.sent_by),
                #[cfg(feature = "codegen")]
                reply: message.options.as_ref().and_then(|options| options.reply.clone()),
                fields: message.field.iter().filter_map(|field| read_field(field, &index)).collect(),
            })
            .collect();
        let wire = index.get(".chatkord.v1.WireMessage").copied().unwrap_or(usize::MAX);
        #[cfg(feature = "codegen")]
        let enums = files
            .iter()
            .flat_map(|file| &file.enum_type)
            .map(|held| {
                let values = held.value.iter().map(|value| value.name.clone().unwrap_or_default()).collect();
                (held.name.clone().unwrap_or_default(), values)
            })
            .collect();
        Self {
            messages,
            wire,
            #[cfg(feature = "codegen")]
            enums,
        }
    }

    /// The message `WireMessage` carries under `number`, if it is an op.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn op(&self, number: u32) -> Option<usize> {
        match self.messages.get(self.wire)?.fields.get(&number)?.shape {
            Shape::Message(held) => held,
            _ => None,
        }
    }
}

fn read_field(field: &DescribedField, index: &HashMap<String, usize>) -> Option<(u32, Field)> {
    let number = u32::try_from(field.number?).ok()?;
    // descriptor.proto's `Type`: 1 double, 6 fixed64, 16 sfixed64; 2 float, 7 fixed32, 15 sfixed32; 9 string;
    // 12 bytes; 10 group and 11 message; every other a varint.
    let shape = match field.r#type? {
        1 | 6 | 16 => Shape::Fixed64,
        2 | 7 | 15 => Shape::Fixed32,
        9 => Shape::Text,
        12 => Shape::Bytes,
        10 | 11 => Shape::Message(field.type_name.as_deref().and_then(|name| index.get(name).copied())),
        _ => Shape::Varint,
    };
    // descriptor.proto's `Label`: 3 is repeated.
    let repeated = field.label == Some(3);
    Some((
        number,
        Field {
            #[cfg(any(test, feature = "codegen", feature = "testing"))]
            name: field.name.clone().unwrap_or_default(),
            repeated,
            most: field.options.as_ref().and_then(|options| options.most).and_then(|most| usize::try_from(most).ok()),
            shape,
            #[cfg(any(test, feature = "testing"))]
            secret: field.options.as_ref().and_then(|options| options.secret).unwrap_or(false),
            #[cfg(any(test, feature = "codegen", feature = "testing"))]
            oneof: field.oneof_index.filter(|_| !field.proto3_optional.unwrap_or(false)),
        },
    ))
}

/// `message` and every message nested in it, by full name.
fn gather<'a>(scope: &str, message: &'a DescriptorMessage, into: &mut Vec<(String, &'a DescriptorMessage)>) {
    let name = format!("{scope}.{}", message.name.as_deref().unwrap_or_default());
    for nested in &message.nested_type {
        gather(&name, nested, into);
    }
    into.push((name, message));
}

#[cfg(test)]
mod tests {
    use super::{SCHEMA, Shape};

    #[test]
    fn the_schema_is_read_whole_with_its_secrets() {
        let schema = &*SCHEMA;
        assert!(schema.messages.len() > 50 && schema.wire != usize::MAX, "{} messages", schema.messages.len());
        let secrets: Vec<String> = schema
            .messages
            .iter()
            .flat_map(|message| {
                message
                    .fields
                    .values()
                    .filter(|field| field.secret)
                    .map(|field| format!("{}.{}", message.name, field.name))
            })
            .collect();
        assert!(secrets.contains(&".chatkord.v1.PasswordCredential.password".to_string()), "{secrets:?}");
        assert!(secrets.contains(&".chatkord.v1.LoginAck.session_token".to_string()), "{secrets:?}");
        assert!(!secrets.iter().any(|name| name.ends_with(".identity")), "{secrets:?}");
        let send = schema.messages.iter().find(|message| message.name == ".chatkord.v1.SendMessage").expect("a send");
        assert!(send.fields.values().any(|field| field.name == "spans" && field.most == Some(200)), "a list's most");
        let login = schema.messages.iter().find(|message| message.name == ".chatkord.v1.Login").expect("Login");
        assert!(
            login.fields.values().any(|field| matches!(field.shape, Shape::Message(Some(_))) && field.oneof.is_some())
        );
    }

    #[test]
    fn a_most_is_stated_only_on_a_repeated_field_of_messages() {
        // The walk counts a list by its length-delimited entries, which a packed list of numbers is not.
        for message in &SCHEMA.messages {
            for field in message.fields.values().filter(|field| field.most.is_some()) {
                assert!(
                    field.repeated && matches!(field.shape, Shape::Message(Some(_))),
                    "{}.{} states a most",
                    message.name,
                    field.name
                );
            }
        }
    }
}
