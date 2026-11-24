//! The report's text, its file name and the bound the caps give it.

use std::time::{Duration, UNIX_EPOCH};

use report::{
    Build, Full, KEPT, MAX_LABEL, MAX_REPORT_BYTES, MAX_ROWS, MAX_SECTIONS, MAX_TITLE, Product, Report, Ring, Section,
    Side, Stands, file_name, note,
};
use tracing_subscriber::layer::SubscriberExt;

const BUILD: Build = Build { program: "Chatkord", version: "0.9.0", source: "tag v0.9.0" };
const MADE: u64 = 1_763_376_120;

#[expect(clippy::unwrap_used, reason = "a test's fixture; each row and section is under its cap")]
fn sample() -> Report {
    let mut stands = Stands::new();
    let mut report = Report::new(Side::Client, BUILD, MADE);
    let mut system = Section::new("System");
    system.push("Session", &Product::parse("Wayland")).unwrap();
    system.push("Threads", &8_usize).unwrap();
    system.push("Container", &false).unwrap();
    report.section(system).unwrap();
    let mut servers = Section::new("Servers");
    servers.push("Signed in", &stands.of("server", "server.example.invalid")).unwrap();
    report.section(servers).unwrap();
    report.section(Section::absent("Voice")).unwrap();
    report
}

fn ring() -> Ring {
    fn two_seconds_on() -> std::time::SystemTime {
        UNIX_EPOCH + Duration::from_secs(MADE + 2)
    }
    Ring::new(two_seconds_on)
}

#[test]
fn the_text_is_the_canvas_shape_and_the_same_each_time() {
    let ring = ring();
    tracing::subscriber::with_default(tracing_subscriber::registry().with(ring.layer()), || {
        note!(info, "joined a voice place");
    });
    let mut report = sample();
    report.log(ring.recent());
    let text = report.text();
    // A note is said with the crate it was made in: this test binary.
    let noted = format!("  10:42:02 info  {} joined a voice place", module_path!().split("::").next().unwrap());
    let expected = [
        "Chatkord debug report: client",
        "Created 2025-11-17 10:42 UTC \u{b7} Chatkord 0.9.0 (tag v0.9.0)",
        "",
        "System",
        "  Session: Wayland",
        "  Threads: 8",
        "  Container: false",
        "",
        "Servers",
        "  Signed in: server-1",
        "",
        "Voice",
        "  not available in this report",
        "",
        "Recent log, newest last",
        "  notes too long to keep: 0",
        &noted,
        "",
    ]
    .join("\n");
    assert_eq!(text, expected);
    assert_eq!(text, report.text());
    assert!(!text.contains("example.invalid"));
}

#[test]
fn the_file_is_named_by_side_and_time() {
    assert_eq!(file_name(Side::Client, 5), "chatkord-client-report-5.txt");
    assert_eq!(file_name(Side::Server, 6), "chatkord-server-report-6.txt");
    assert_eq!(sample().file_name(), "chatkord-client-report-1763376120.txt");
}

#[test]
fn every_list_is_capped_where_it_is_built() {
    let long_label: &'static str = "l".repeat(MAX_LABEL + 1).leak();
    let long_title: &'static str = "t".repeat(MAX_TITLE + 1).leak();
    let mut section = Section::new("Rows");
    for _ in 0..MAX_ROWS {
        section.push("row", &1_u8).unwrap();
    }
    assert_eq!(section.push("row", &1_u8), Err(Full::Rows));
    assert_eq!(Section::new("x").push("", &1_u8), Err(Full::Name));
    assert_eq!(Section::new("x").push(long_label, &1_u8), Err(Full::Name));
    let mut report = Report::new(Side::Server, BUILD, MADE);
    for _ in 0..MAX_SECTIONS {
        report.section(Section::new("Section")).unwrap();
    }
    assert_eq!(report.section(Section::new("Section")), Err(Full::Sections));
    assert_eq!(Report::new(Side::Server, BUILD, MADE).section(Section::new("")), Err(Full::Name));
    assert_eq!(Report::new(Side::Server, BUILD, MADE).section(Section::new(long_title)), Err(Full::Name));
}

#[test]
fn the_fullest_report_the_caps_allow_stays_under_the_bound() {
    let widest = Product::parse(&"p".repeat(96));
    let label: &'static str = "l".repeat(MAX_LABEL).leak();
    let title: &'static str = "t".repeat(MAX_TITLE).leak();
    let mut report = Report::new(Side::Client, BUILD, u64::MAX / 2);
    for _ in 0..MAX_SECTIONS {
        let mut section = Section::new(title);
        for _ in 0..MAX_ROWS {
            section.push(label, &widest).unwrap();
        }
        report.section(section).unwrap();
    }
    let ring = ring();
    tracing::subscriber::with_default(tracing_subscriber::registry().with(ring.layer()), || {
        for _ in 0..(KEPT * 2) {
            note!(
                error,
                "a note that takes room, and takes it by being long: a long long long long long long long long long long long long long sentence",
                a = u64::MAX,
                b = u64::MAX,
                c = u64::MAX,
                d = true
            );
        }
    });
    report.log(ring.recent());
    let size = report.text().len();
    assert!(size <= MAX_REPORT_BYTES, "{size} bytes");
    assert!(size > MAX_REPORT_BYTES / 3, "the test fills the caps: {size} bytes");
}

fn with_notes(count: usize, dropped_long: bool) -> Report {
    let ring = ring();
    tracing::subscriber::with_default(tracing_subscriber::registry().with(ring.layer()), || {
        for at in 0..count {
            note!(info, "a share started", frames = at);
        }
        if dropped_long {
            note!(
                info,
                "a note too long to keep, so it is counted and not kept: long long long long long long long long long long long long long long long long long long long long long long long long long long long long long long long long long long long long long long long long"
            );
        }
    });
    let mut report = sample();
    report.log(ring.recent());
    report
}

#[test]
fn a_listing_made_of_the_parts_is_the_listing_read_from_the_text() {
    for (count, newest) in [(0, 5), (3, 5), (5, 5), (40, 5), (40, 0)] {
        let report = with_notes(count, true);
        let read = report::Listing::read(&report.text(), newest);
        assert_eq!(read.as_ref(), Ok(&report.listing(newest)), "{count} notes, {newest} listed");
    }
    let listing = with_notes(40, false).listing(5);
    let log = listing.parts().last().and_then(report::ListedPart::log).expect("the log is the last part");
    assert_eq!((log.total(), log.newest().len()), (40, 5));
    assert!(log.newest().last().is_some_and(|line| line.ends_with("frames=39")));
}

#[test]
fn a_text_that_is_not_what_text_writes_is_not_read() {
    let good = with_notes(3, false).text();
    assert!(report::Listing::read(&good, 5).is_ok());
    let cases = [
        good.trim_end().to_owned(),
        good.replacen("\n\n", "\n", 1),
        good.replacen("Chatkord debug report: client", "Chatkord debug report: other", 1),
        good.replace("  notes too long to keep: 0", "  notes too long to keep: many"),
        format!("{good}\nRecent log, newest last\n  notes too long to keep: 0\n"),
        format!("{good}\nSection\nno indent\n"),
        format!("{good}\n\nSection\n"),
        "x".repeat(MAX_REPORT_BYTES + 1),
        "\n\n".repeat(MAX_SECTIONS + 3),
        good.replace("  notes too long to keep: 0", "  notes too long to keep: 00"),
        good.replace("  notes too long to keep: 0", "  notes too long to keep: +0"),
        good.replacen("Created ", &format!("Created {}", "9".repeat(4096)), 1),
        good.replacen("  Threads: 8", &format!("  Threads: {}", "8".repeat(4096)), 1),
        format!("{good}  {}\n", "x".repeat(4096)),
    ];
    for text in cases {
        assert!(report::Listing::read(&text, 5).is_err(), "{text:?}");
    }
    let many = format!("Chatkord debug report: client\nCreated now\n{}", "\nSection\n  row\n".repeat(MAX_SECTIONS + 3));
    assert_eq!(report::Listing::read(&many, 5), Err(report::NotAReport::Shape));
}

#[test]
fn an_absent_section_holds_no_rows_and_takes_none() {
    let mut absent = Section::absent("Voice");
    assert_eq!(absent.push("row", &1_u8), Err(Full::Rows));
    assert_eq!(absent.row_count(), 0);
}

#[test]
fn a_running_servers_empty_log_is_listed_with_its_count() {
    let mut report = Report::new(Side::Server, BUILD, MADE);
    report.log(ring().recent());
    assert!(report.text().ends_with("\nRecent log, newest last\n  notes too long to keep: 0\n"), "{}", report.text());
}

#[test]
fn a_build_word_over_its_bound_takes_no_section() {
    let long: &'static str = "v".repeat(report::MAX_BUILD_WORD + 1).leak();
    let build = Build { program: "Chatkord", version: long, source: "tag" };
    assert_eq!(Report::new(Side::Client, build, MADE).section(Section::new("System")), Err(Full::Name));
}
