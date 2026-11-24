//! `Debug` for the generated types that carry a secret, which `wire/buf.gen.yaml` keeps from deriving one: every field
//! in schema order, a secret as `[redacted]`. `Login`'s derived one is safe because its credentials redact.

use crate::chatkord_v1::{
    ChangePassword, InviteCreated, InviteInfo, LeaveServer, LoginAck, PasswordCredential, PasswordReset, Register,
    ServerReport, SetBanned, TokenCredential, TransferOwnership, WebhookMade,
};
use std::fmt;

// The word every secret type prints.
const REDACTED: &str = "[redacted]";

struct Redacted;

impl fmt::Debug for Redacted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED)
    }
}

impl fmt::Debug for PasswordCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PasswordCredential").field("password", &Redacted).finish()
    }
}

impl fmt::Debug for TokenCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokenCredential").field("token", &Redacted).finish()
    }
}

impl fmt::Debug for LoginAck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoginAck")
            .field("session_token", &self.session_token.as_ref().map(|_| Redacted))
            .field("display_name", &self.display_name)
            .field("identity", &self.identity)
            .field("place", &self.place)
            .field("must_change_password", &self.must_change_password)
            .field("grants", &self.grants)
            .field("user_id", &self.user_id)
            .field("colour", &self.colour)
            .finish()
    }
}

impl fmt::Debug for Register {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Register")
            .field("identity", &self.identity)
            .field("display_name", &self.display_name)
            .field("password", &Redacted)
            .field("invite_code", &Redacted)
            .finish()
    }
}

impl fmt::Debug for ChangePassword {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChangePassword").field("password", &Redacted).field("current", &Redacted).finish()
    }
}

impl fmt::Debug for InviteCreated {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InviteCreated").field("code", &Redacted).field("expires_at", &self.expires_at).finish()
    }
}

impl fmt::Debug for InviteInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InviteInfo").field("code", &Redacted).finish()
    }
}

impl fmt::Debug for PasswordReset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PasswordReset").field("temporary_password", &Redacted).finish()
    }
}

impl fmt::Debug for LeaveServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LeaveServer")
            .field("current", &Redacted)
            .field("delete_messages", &self.delete_messages)
            .finish()
    }
}

impl fmt::Debug for TransferOwnership {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TransferOwnership").field("user_id", &self.user_id).field("current", &Redacted).finish()
    }
}

impl fmt::Debug for ServerReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerReport").field("id", &Redacted).finish()
    }
}

impl fmt::Debug for WebhookMade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebhookMade").field("webhook", &self.webhook).field("token", &Redacted).finish()
    }
}

impl fmt::Debug for SetBanned {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SetBanned")
            .field("user_id", &self.user_id)
            .field("banned", &self.banned)
            .field("current", &Redacted)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use crate::chatkord_v1::WireMessage;
    use crate::filled::{every_op_filled, marked_secrets};
    use std::collections::BTreeSet;

    const SECRET: &str = "hunter2-secret";

    #[test]
    fn every_field_the_schema_marks_secret_prints_redacted_and_every_other_text_prints() {
        let mut reached = BTreeSet::new();
        for filled in every_op_filled(SECRET) {
            let shown = format!("{:?}", WireMessage { body: Some(filled.op), ..WireMessage::default() });
            assert!(!shown.contains(SECRET), "{} prints a secret: {shown}", filled.name);
            for plain in &filled.plain {
                assert!(shown.contains(plain.as_str()), "{} leaves {plain} out: {shown}", filled.name);
            }
            reached.extend(filled.secrets);
        }
        let marked = marked_secrets();
        assert!(!marked.is_empty(), "the schema marks no secret");
        assert_eq!(reached, marked, "every secret is reached from an op");
    }
}
