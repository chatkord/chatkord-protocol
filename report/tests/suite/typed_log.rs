//! The typed log end to end: what a note and an operator line do to the ring and to another layer's output.

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use report::{Fact, KEPT, MAX_LINE, Product, Public, Ring, Stands, note, operator, word};
use tracing_subscriber::layer::SubscriberExt;

fn fixed_clock() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_763_376_122)
}

#[expect(dead_code, reason = "the held text is what a derive must never print")]
#[derive(Debug, Public)]
enum Why {
    NoDevice,
    Refused(String),
    Late {
        ms: u32,
    },
    Status(#[public] u16),
    Coded {
        #[public]
        code: i32,
        says: &'static str,
    },
    Nested(#[public] Option<Box<Why>>),
}

impl Public for Box<Why> {
    fn fact(&self) -> Fact {
        (**self).fact()
    }
}

fn within<T>(ring: &Ring, run: impl FnOnce() -> T) -> T {
    tracing::subscriber::with_default(tracing_subscriber::registry().with(ring.layer()), run)
}

#[test]
fn a_note_is_kept_as_its_literal_and_its_facts() {
    let ring = Ring::new(fixed_clock);
    let why = Why::Refused("/scratch/secret.txt".to_string());
    within(&ring, || {
        note!(warn, "a decoder fell back to software", codec = 1_u8, hardware = false, why = why);
        note!(info, "joined");
    });
    let lines = ring.lines();
    // A note is said with the crate it was made in: this test binary.
    let made_in = module_path!().split("::").next().unwrap();
    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[0].as_str(),
        format!("10:42:02 warn  {made_in} a decoder fell back to software  codec=1  hardware=false  why=Refused")
    );
    assert_eq!(lines[1].as_str(), format!("10:42:02 info  {made_in} joined"));
}

#[test]
fn an_enum_is_public_by_its_variant_name_and_never_by_what_it_holds() {
    let why = [Why::NoDevice, Why::Refused("/scratch".into()), Why::Late { ms: 7 }];
    let words: Vec<String> = why.iter().map(|why| why.fact().to_string()).collect();
    assert_eq!(words, ["NoDevice", "Refused", "Late"]);
}

#[test]
fn an_operator_line_is_never_kept_and_a_note_is_never_lost_to_it() {
    let ring = Ring::new(fixed_clock);
    let peer = std::net::IpAddr::from([192, 0, 2, 7]);
    within(&ring, || {
        operator!(info, "a peer connected", peer = peer);
        note!(info, "a socket opened", open = 1_u32);
    });
    let lines = ring.lines();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].as_str().contains("a socket opened") && !lines[0].as_str().contains("192"));
}

#[test]
fn another_layer_sees_both_kinds() {
    struct Seen(Arc<Mutex<Vec<String>>>);
    impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Seen {
        fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
            if let Ok(mut seen) = self.0.lock() {
                seen.push(event.metadata().target().to_string());
            }
        }
    }
    let seen = Arc::new(Mutex::new(Vec::new()));
    let ring = Ring::new(fixed_clock);
    let subscriber = tracing_subscriber::registry().with(Seen(seen.clone())).with(ring.layer());
    tracing::subscriber::with_default(subscriber, || {
        note!(error, "a note");
        operator!(error, "an operator line");
    });
    assert_eq!(*seen.lock().unwrap(), [module_path!(), module_path!()]);
    assert_eq!(ring.lines().len(), 1);
}

#[test]
fn the_ring_is_bounded_and_drops_the_oldest() {
    let ring = Ring::new(fixed_clock);
    within(&ring, || {
        for number in 0..(KEPT + 50) {
            note!(info, "tick", number = number);
        }
    });
    let lines = ring.lines();
    assert_eq!(lines.len(), KEPT);
    assert!(lines[0].as_str().ends_with("number=50"));
    assert!(lines[KEPT - 1].as_str().ends_with(&format!("number={}", KEPT + 49)));
    assert!(lines.iter().all(|line| line.as_str().len() <= MAX_LINE));
}

#[test]
fn a_handle_is_the_same_ring() {
    let ring = Ring::new(fixed_clock);
    let handle = ring.clone();
    within(&ring, || note!(info, "seen"));
    assert_eq!(handle.lines().len(), 1);
}

#[test]
fn a_product_a_stand_in_and_a_refused_name_print_as_facts() {
    let ring = Ring::new(fixed_clock);
    let gpu = Product::fact_of("AMD Radeon RX 9060 XT");
    let refused = Product::fact_of("/scratch");
    let stand = Stands::new().of("member", "ada");
    within(&ring, || note!(info, "found", gpu = gpu, odd = refused, member = stand));
    assert!(ring.lines()[0].as_str().ends_with("gpu=AMD Radeon RX 9060 XT  odd=withheld  member=member-1"));
}

#[test]
fn a_marked_field_is_shown_and_an_unmarked_one_is_not() {
    let facts = [Why::Status(404), Why::Coded { code: -3, says: "/scratch/secret" }, Why::Refused("/scratch".into())];
    let shown: Vec<String> = facts.iter().map(|why| why.fact().to_string()).collect();
    assert_eq!(shown, ["Status(404)", "Coded(-3)", "Refused"]);
    assert_eq!(Why::Nested(Some(Box::new(Why::Status(7)))).fact().to_string(), "Nested(Status)");
    assert_eq!(Why::Nested(None).fact(), Fact::case(word!("Nested"), Fact::Absent));
}

#[test]
fn a_note_below_info_is_not_kept_and_the_warnings_stay() {
    let ring = Ring::new(fixed_clock);
    within(&ring, || {
        note!(warn, "a warning");
        for _ in 0..(KEPT * 2) {
            note!(debug, "a refused op");
            note!(trace, "a step");
        }
    });
    let lines = ring.lines();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].as_str().ends_with("a warning"));
}

#[test]
fn a_foreign_event_named_note_is_not_kept() {
    let ring = Ring::new(fixed_clock);
    within(&ring, || {
        tracing::event!(name: "note", tracing::Level::ERROR, path = "/scratch/secret", "a foreign event");
        note!(error, "ours");
    });
    let lines = ring.lines();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].as_str().ends_with("ours"));
}

#[test]
fn a_note_too_long_to_keep_is_counted() {
    let ring = Ring::new(fixed_clock);
    let long = Product::parse(&"a".repeat(90));
    within(&ring, || {
        note!(info, "too long", one = long, two = long, three = long);
        note!(info, "fits");
    });
    assert_eq!(ring.lines().len(), 1);
    assert_eq!(ring.recent().dropped(), 1);
}
