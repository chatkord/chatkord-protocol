//! What both ends know about a file: its id, its kind and that kind's bounds, the types a picture
//! may be by its first bytes, and the status each refusal of `POST /v1/files` and `GET /v1/files/<id>` is answered
//! with. The server refuses by them; the client checks and reads by them.

/// A file's id: 128 random bits, one per upload, written as 32 lower-case hexadecimal digits wherever it travels, so
/// one id is never two strings. It names a file and grants nothing: a download is authorised by what refers to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileId([u8; FileId::LEN]);

/// Why a written file id was refused: anything but 32 lower-case hexadecimal digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileIdRefused;

impl FileId {
    /// The id's length in bytes.
    pub const LEN: usize = 16;

    /// The id these bytes are.
    pub const fn from_bytes(bytes: [u8; FileId::LEN]) -> FileId {
        FileId(bytes)
    }

    /// The id's bytes, as the store keeps them.
    pub const fn as_bytes(&self) -> &[u8; FileId::LEN] {
        &self.0
    }

    /// Exactly 32 lower-case hexadecimal digits; anything else, of any length, is refused whole.
    pub fn parse(text: &str) -> Result<FileId, FileIdRefused> {
        if text.len() != 2 * FileId::LEN || !text.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')) {
            return Err(FileIdRefused);
        }
        let mut bytes = [0u8; FileId::LEN];
        for (byte, pair) in bytes.iter_mut().zip(text.as_bytes().as_chunks::<2>().0) {
            let pair = std::str::from_utf8(pair).map_err(|_| FileIdRefused)?;
            *byte = u8::from_str_radix(pair, 16).map_err(|_| FileIdRefused)?;
        }
        Ok(FileId(bytes))
    }
}

impl std::fmt::Display for FileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}

/// The one-use id a server answers a report ask with: a token-shaped secret that prints `[redacted]`, zeroed when
/// dropped, and appears only in the download's path.
pub struct ReportId(zeroize::Zeroizing<String>);

/// Why a written report id was refused: anything but [`crate::rules::TOKEN_LEN`] characters of base64url.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReportIdRefused;

impl ReportId {
    /// `text` as an id, refused whole where it is not token-shaped, so no other text reaches a request's path.
    pub fn parse(text: String) -> Result<ReportId, ReportIdRefused> {
        let text = zeroize::Zeroizing::new(text);
        if crate::rules::token_shaped(&text) { Ok(ReportId(text)) } else { Err(ReportIdRefused) }
    }

    /// The path the report is downloaded from: the one place the id is read.
    pub fn path(&self) -> String {
        format!("/v1/files/reports/{}", *self.0)
    }
}

impl std::fmt::Debug for ReportId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[redacted]")
    }
}

const KIB: u64 = 1024;
const MIB: u64 = 1024 * KIB;

/// A picture sent to be a server's or a member's picture: at most this many bytes, refused before it is read.
pub const PICTURE_UPLOAD_BYTES: u64 = 2 * MIB;

/// A picture sent may be at most this many pixels on a side, read from its header before it is decoded.
pub const PICTURE_UPLOAD_SIDE: u32 = 4096;

/// A kept picture is a PNG of this many pixels on a side, made by the server.
pub const PICTURE_SIDE: u32 = 256;

/// A kept picture is at most this many bytes: more than a 256-pixel RGBA PNG can take, so a server sending more is not
/// sending one of ours.
pub const PICTURE_STORED_BYTES: u64 = 512 * KIB;

/// The most any server takes in one attachment, whatever its operator sets: the bound a client refuses before sending
/// and reads a download to. A server's own most is its setting, at or below this.
pub const ATTACHMENT_MOST_BYTES: u64 = 4 * 1024 * MIB;

/// The most one picture's decode may hold at either end: a client's read of a picture it shows, and the ceiling of a
/// server's intake of one, whose operator may set less.
pub const PICTURE_DECODE_BYTES: u64 = 256 * MIB;

/// The sides a thumbnail fits within, never larger than its picture (Synapse's largest scale).
pub const THUMBNAIL_BOX: (u32, u32) = (800, 600);

/// A thumbnail is at most this many bytes: more than an 800 x 600 RGBA PNG can take.
pub const THUMBNAIL_STORED_BYTES: u64 = 4 * MIB;

/// An emoji sent: at most this many bytes, refused before it is read.
pub const EMOJI_UPLOAD_BYTES: u64 = 256 * KIB;

/// A kept emoji is a PNG of this many pixels on a side, made by the server.
pub const EMOJI_SIDE: u32 = 128;

/// A kept emoji is at most this many bytes: more than a 128-pixel RGBA PNG can take.
pub const EMOJI_STORED_BYTES: u64 = 128 * KIB;

/// What a file is for, which decides its bounds, its intake and who may upload and see it. The store's
/// `file_kinds` holds the same words, written from [`FileKind::ALL`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FileKind {
    /// The server's picture: uploaded by a holder of `MANAGE_SERVER`, seen by every member once it is set.
    ServerPicture,
    /// A file a member attaches to a message, kept as sent but a picture's metadata; seen by whoever reads the
    /// timeline of a message it is attached to.
    Attachment,
    /// The server's smaller picture of an attached picture, made at its intake and never uploaded; seen as its
    /// picture is.
    Thumbnail,
    /// A custom emoji's picture: uploaded by a holder of `MANAGE_EMOJI`, seen by every member.
    Emoji,
    /// A member's own picture: uploaded by any member for itself, seen by every member once it is set; bounded and made
    /// as a server's picture is.
    ProfilePicture,
}

impl FileKind {
    /// Every kind, in a fixed order.
    pub const ALL: [FileKind; 5] =
        [FileKind::ServerPicture, FileKind::Attachment, FileKind::Thumbnail, FileKind::Emoji, FileKind::ProfilePicture];

    /// The kind's word, as the store keeps it and an upload names it.
    pub const fn word(self) -> &'static str {
        match self {
            FileKind::ServerPicture => "server-picture",
            FileKind::Attachment => "attachment",
            FileKind::Thumbnail => "thumbnail",
            FileKind::Emoji => "emoji",
            FileKind::ProfilePicture => "profile-picture",
        }
    }

    /// The kind `word` names, or none.
    pub fn from_word(word: &str) -> Option<FileKind> {
        FileKind::ALL.into_iter().find(|kind| kind.word() == word)
    }

    /// Whether a person sends files of this kind; one the server makes itself is never taken in.
    pub const fn uploaded(self) -> bool {
        !matches!(self, FileKind::Thumbnail)
    }

    /// The most bytes one upload of this kind may be; zero for a kind nobody uploads. An attachment's is the most any
    /// server takes: each server's own is its operator's setting.
    pub const fn upload_bytes(self) -> u64 {
        match self {
            FileKind::ServerPicture | FileKind::ProfilePicture => PICTURE_UPLOAD_BYTES,
            FileKind::Attachment => ATTACHMENT_MOST_BYTES,
            FileKind::Thumbnail => 0,
            FileKind::Emoji => EMOJI_UPLOAD_BYTES,
        }
    }

    /// The most bytes a kept file of this kind may be, so a download past it is refused rather than read.
    pub const fn stored_bytes(self) -> u64 {
        match self {
            FileKind::ServerPicture | FileKind::ProfilePicture => PICTURE_STORED_BYTES,
            FileKind::Attachment => ATTACHMENT_MOST_BYTES,
            FileKind::Thumbnail => THUMBNAIL_STORED_BYTES,
            FileKind::Emoji => EMOJI_STORED_BYTES,
        }
    }
}

/// The most bytes an attachment's name may be, as sent and as kept.
pub const ATTACHMENT_NAME_BYTES: usize = 255;

/// An attachment's name as its sender gave it: 1 to [`ATTACHMENT_NAME_BYTES`] bytes of UTF-8, no control or
/// bidirectional formatting character (which could make `evil.exe` read as `exe.png`), no `/` or `\`, and never `.` or
/// `..`. A name only: it is shown and offered when saving, never a path.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AttachmentName(String);

/// Why an attachment's name was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameRefused {
    /// Empty, or more than [`ATTACHMENT_NAME_BYTES`] bytes.
    Length,
    /// A control or bidirectional formatting character, a path separator, or a name that is only dots.
    Character,
    /// Percent-encoding that is malformed or decodes to something not UTF-8.
    Encoding,
}

impl AttachmentName {
    /// `text` as a name, refused whole when it breaks any rule.
    pub fn parse(text: &str) -> Result<AttachmentName, NameRefused> {
        if text.is_empty() || text.len() > ATTACHMENT_NAME_BYTES {
            return Err(NameRefused::Length);
        }
        if text.chars().any(|c| c.is_control() || bidirectional(c) || matches!(c, '/' | '\\'))
            || matches!(text, "." | "..")
        {
            return Err(NameRefused::Character);
        }
        Ok(AttachmentName(text.to_string()))
    }

    /// The name `encoded` percent-encodes (a query's `name`), decoded and parsed; a `+` is itself, never a space.
    pub fn percent_decoded(encoded: &str) -> Result<AttachmentName, NameRefused> {
        if encoded.len() > 3 * ATTACHMENT_NAME_BYTES {
            return Err(NameRefused::Length);
        }
        let mut bytes = Vec::with_capacity(encoded.len());
        let mut rest = encoded.as_bytes();
        while let Some((&byte, after)) = rest.split_first() {
            if byte == b'%' {
                let [high, low, after @ ..] = after else { return Err(NameRefused::Encoding) };
                let digit = |byte: u8| char::from(byte).to_digit(16).ok_or(NameRefused::Encoding);
                bytes.push(u8::try_from(digit(*high)? * 16 + digit(*low)?).map_err(|_| NameRefused::Encoding)?);
                rest = after;
            } else {
                bytes.push(byte);
                rest = after;
            }
        }
        AttachmentName::parse(std::str::from_utf8(&bytes).map_err(|_| NameRefused::Encoding)?)
    }

    /// The name percent-encoded, every byte but RFC 8187's `attr-char` escaped: fit for a query's `name` and for
    /// `Content-Disposition`'s `filename*=UTF-8''…`.
    pub fn percent_encoded(&self) -> String {
        let mut encoded = String::with_capacity(self.0.len());
        for byte in self.0.bytes() {
            if byte.is_ascii_alphanumeric() || b"!#$&+-.^_`|~".contains(&byte) {
                encoded.push(char::from(byte));
            } else {
                encoded.push_str(&format!("%{byte:02X}"));
            }
        }
        encoded
    }

    /// The name as given.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Unicode's bidirectional formatting characters (UAX #9: the marks, embeddings, overrides and isolates).
const fn bidirectional(c: char) -> bool {
    matches!(c, '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// What a kept file's bytes are, as the store keeps it and a download is labelled: one of the pictures the server
/// reads, or bytes it does not judge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MediaType {
    /// A picture the server read as this type by its first bytes and its structure.
    Picture(PictureType),
    /// Anything else, labelled so no reader guesses a type from it.
    Bytes,
}

impl MediaType {
    /// Every type, in a fixed order.
    pub const ALL: [MediaType; 4] = [
        MediaType::Picture(PictureType::Png),
        MediaType::Picture(PictureType::Jpeg),
        MediaType::Picture(PictureType::WebP),
        MediaType::Bytes,
    ];

    /// The type's name, as the store keeps it, a download is labelled with and an attachment states.
    pub const fn word(self) -> &'static str {
        match self {
            MediaType::Picture(PictureType::Png) => "image/png",
            MediaType::Picture(PictureType::Jpeg) => "image/jpeg",
            MediaType::Picture(PictureType::WebP) => "image/webp",
            MediaType::Bytes => "application/octet-stream",
        }
    }

    /// The type `word` names, or none.
    pub fn from_word(word: &str) -> Option<MediaType> {
        MediaType::ALL.into_iter().find(|kind| kind.word() == word)
    }

    /// The picture it is, or none.
    pub const fn picture(self) -> Option<PictureType> {
        match self {
            MediaType::Picture(picture) => Some(picture),
            MediaType::Bytes => None,
        }
    }
}

/// The types a picture is taken in, judged by its first bytes, never by a name or a header a client sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PictureType {
    /// Portable Network Graphics.
    Png,
    /// JPEG (JFIF or Exif).
    Jpeg,
    /// WebP, lossy or lossless.
    WebP,
}

impl PictureType {
    /// What `bytes` say they are by their signature, or none.
    pub fn sniff(bytes: &[u8]) -> Option<PictureType> {
        match bytes {
            [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, ..] => Some(PictureType::Png),
            [0xFF, 0xD8, 0xFF, ..] => Some(PictureType::Jpeg),
            [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Some(PictureType::WebP),
            _ => None,
        }
    }
}

/// Why a file request was refused, each with the one status it is answered with; a refusal never has a body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileRefusal {
    /// No token, or one that signs nobody in: one refusal whatever was wrong with it.
    Unauthenticated,
    /// Signed in, and the kind's grant word is not held.
    NotAuthorized,
    /// No such file, or one this account may not see: the same answer.
    NotFound,
    /// The request is not one these routes take (an unknown kind, an attachment's name missing or refused, a body that
    /// is not its stated length).
    BadRequest,
    /// An upload that does not state its length.
    LengthRequired,
    /// Over the kind's size.
    TooLarge,
    /// The server has no room left: its disk or its store is full.
    Full,
    /// The sender's own files would pass what one account may keep.
    AccountFull,
    /// Not a type the kind takes, by its first bytes.
    UnsupportedType,
    /// A picture with too many pixels, or one that could not be read as what it said it was.
    Unprocessable,
    /// Too many requests on one connection, or uploads by one account.
    TooMany,
    /// The server holds as many connections as it may: none is left for this one.
    Unavailable,
    /// The store, the disk or the clock failed.
    Internal,
}

impl FileRefusal {
    /// Every refusal, in a fixed order.
    pub const ALL: [FileRefusal; 13] = [
        Self::BadRequest,
        Self::Unauthenticated,
        Self::NotAuthorized,
        Self::NotFound,
        Self::LengthRequired,
        Self::TooLarge,
        Self::UnsupportedType,
        Self::Unprocessable,
        Self::TooMany,
        Self::Unavailable,
        Self::Internal,
        Self::Full,
        Self::AccountFull,
    ];

    /// The HTTP status this refusal is answered with.
    pub const fn status(self) -> u16 {
        match self {
            Self::BadRequest => 400,
            Self::Unauthenticated => 401,
            Self::NotAuthorized => 403,
            Self::NotFound => 404,
            Self::LengthRequired => 411,
            Self::TooLarge => 413,
            Self::UnsupportedType => 415,
            Self::Unprocessable => 422,
            Self::TooMany => 429,
            Self::Unavailable => 503,
            Self::Internal => 500,
            Self::Full => 507,
            // No status is an account's quota; 409 is the request at odds with its sender's state.
            Self::AccountFull => 409,
        }
    }

    /// The refusal a status answers, or none for one these routes never send.
    pub fn of_status(status: u16) -> Option<FileRefusal> {
        FileRefusal::ALL.into_iter().find(|refusal| refusal.status() == status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_id_is_thirty_two_lower_case_hex_digits_written_one_way() {
        let id = FileId::from_bytes([0x0f, 0xa1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0x10]);
        let written = id.to_string();
        assert_eq!(written, "0fa1000000000000000000000000ff10");
        assert_eq!(FileId::parse(&written), Ok(id));
        for bad in [
            "",
            "0fa1",
            "0FA1000000000000000000000000FF10",
            "0fa1000000000000000000000000ff1",
            "0fa1000000000000000000000000ff100",
            "0fa1000000000000000000000000ff1g",
            "+fa1000000000000000000000000ff10",
            "../a1000000000000000000000000ff10",
        ] {
            assert_eq!(FileId::parse(bad), Err(FileIdRefused), "{bad:?}");
        }
    }

    #[test]
    fn a_file_kind_is_one_word_each_way() {
        for kind in FileKind::ALL {
            assert_eq!(FileKind::from_word(kind.word()), Some(kind));
        }
        assert_eq!(FileKind::from_word("Server-Picture"), None);
    }

    #[test]
    fn a_status_names_one_refusal_and_back() {
        for refusal in FileRefusal::ALL {
            assert_eq!(FileRefusal::of_status(refusal.status()), Some(refusal), "{refusal:?}");
        }
        assert_eq!(FileRefusal::of_status(200), None);
        assert_eq!(FileRefusal::of_status(201), None);
    }

    #[test]
    fn an_attachment_s_name_is_bounded_and_never_a_path_or_a_disguise() {
        for good in ["photo.jpg", "a b+c.tar.gz", "Ελληνικά.pdf", "...hidden", &"x".repeat(ATTACHMENT_NAME_BYTES)]
        {
            assert_eq!(AttachmentName::parse(good).map(|name| name.as_str().to_string()), Ok(good.to_string()));
        }
        assert_eq!(AttachmentName::parse(""), Err(NameRefused::Length));
        assert_eq!(AttachmentName::parse(&"x".repeat(ATTACHMENT_NAME_BYTES + 1)), Err(NameRefused::Length));
        assert_eq!(AttachmentName::parse(&"é".repeat(128)), Err(NameRefused::Length), "bytes, not characters");
        for bad in
            ["a/b", "a\\b", "..", ".", "a\u{0}b", "line\nbreak", "evil\u{202E}gnp.exe", "a\u{2066}b", "a\u{200F}"]
        {
            assert_eq!(AttachmentName::parse(bad), Err(NameRefused::Character), "{bad:?}");
        }
    }

    #[test]
    fn a_name_travels_percent_encoded_and_comes_back_the_same() {
        for name in ["photo 1.jpg", "100%.png", "Ελληνικά & more?.pdf", "a+b=c;d,e\"f'.txt"] {
            let parsed = AttachmentName::parse(name).expect("a name");
            let encoded = parsed.percent_encoded();
            assert!(
                encoded.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"!#$&+-.^_`|~%".contains(&byte)),
                "{encoded}"
            );
            assert_eq!(AttachmentName::percent_decoded(&encoded), Ok(parsed));
        }
        assert_eq!(AttachmentName::percent_decoded("a%2Fb"), Err(NameRefused::Character), "an escaped slash is one");
        assert_eq!(AttachmentName::percent_decoded("a%e2%80%aeb"), Err(NameRefused::Character));
        for bad in ["a%", "a%2", "a%zz", "%ff%fe"] {
            assert_eq!(AttachmentName::percent_decoded(bad), Err(NameRefused::Encoding), "{bad}");
        }
        assert_eq!(AttachmentName::percent_decoded("a+b").map(|name| name.as_str().to_string()), Ok("a+b".into()));
        assert_eq!(AttachmentName::percent_decoded(&"%41".repeat(ATTACHMENT_NAME_BYTES + 1)), Err(NameRefused::Length));
    }

    #[test]
    fn a_media_type_is_one_word_each_way_and_only_a_picture_is_one() {
        for kind in MediaType::ALL {
            assert_eq!(MediaType::from_word(kind.word()), Some(kind));
        }
        assert_eq!(MediaType::Picture(PictureType::WebP).picture(), Some(PictureType::WebP));
        assert_eq!(MediaType::Bytes.picture(), None);
        assert_eq!(MediaType::from_word("image/svg+xml"), None);
    }

    #[test]
    fn only_a_kind_a_person_sends_is_taken_in() {
        for kind in FileKind::ALL {
            assert_eq!(kind.uploaded(), kind.upload_bytes() > 0, "{kind:?}");
            assert!(kind.stored_bytes() > 0, "{kind:?}");
        }
        assert!(!FileKind::Thumbnail.uploaded());
    }

    #[test]
    fn a_picture_is_known_by_its_first_bytes_alone() {
        assert_eq!(PictureType::sniff(b"\x89PNG\r\n\x1a\nrest"), Some(PictureType::Png));
        assert_eq!(PictureType::sniff(b"\xff\xd8\xff\xe0"), Some(PictureType::Jpeg));
        assert_eq!(PictureType::sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some(PictureType::WebP));
        for bad in [&b""[..], b"\x89PNG", b"GIF89a", b"RIFF\0\0\0\0WAVE", b"<svg"] {
            assert_eq!(PictureType::sniff(bad), None, "{bad:?}");
        }
    }
}
