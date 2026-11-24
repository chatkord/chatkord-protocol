//! What a frame from a server costs the client that reads it: every op, each of its fields filled with empty entries
//! up to the frame's bound, read through `read_frame` as a client reads, is refused or decodes into a bounded size.
//! Its own binary, since it counts every allocation the process makes.
#![allow(clippy::expect_used, clippy::panic, reason = "tests: a failed step is the test failing")]

use proto::chatkord_v1::wire_message::Body;
use proto::envelope::{MAX_FRAME_BYTES, every_op, read_frame};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

struct Counting;

// SAFETY: every call goes to `System` unchanged; the count beside it touches no memory the caller is given.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: the caller's contract for `alloc` is `System.alloc`'s.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from `System` through this allocator, with this layout.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATED.fetch_add(new_size, Ordering::Relaxed);
        // SAFETY: `ptr` came from `System` through this allocator, with this layout.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

/// The most a frame from a server may decode into: the entries bound times the largest entry, with room to spare.
const MOST_BYTES: usize = 16 * 1024 * 1024;

fn varint(value: u64, out: &mut Vec<u8>) {
    prost::encode_length_delimiter(usize::try_from(value).expect("fits"), out).expect("a Vec grows");
}

fn number_of(op: &Body) -> u64 {
    let mut bytes = Vec::new();
    op.encode(&mut bytes);
    let mut key = bytes.as_slice();
    prost::decode_length_delimiter(&mut key).expect("a key") as u64 >> 3
}

/// `seq = 7`, then the op whose payload is `payload` bytes of empty length-delimited entries in `field`.
fn frame(op: &Body, field: u32, payload: usize) -> Vec<u8> {
    let mut entry = Vec::new();
    varint(u64::from(field) << 3 | 2, &mut entry);
    entry.push(0);
    let filled: Vec<u8> = entry.iter().copied().cycle().take(payload / entry.len() * entry.len()).collect();
    let mut frame = vec![0x08, 0x07];
    varint(number_of(op) << 3 | 2, &mut frame);
    varint(filled.len() as u64, &mut frame);
    frame.extend_from_slice(&filled);
    frame
}

/// The bytes allocated while the frame is read and what it reads to is dropped.
fn cost(bytes: &[u8]) -> usize {
    let before = ALLOCATED.load(Ordering::Relaxed);
    drop(read_frame(bytes));
    ALLOCATED.load(Ordering::Relaxed) - before
}

#[test]
fn a_servers_frame_of_empty_entries_is_refused_or_decodes_into_a_bounded_size() {
    let mut worst = (0, String::new());
    let mut over = Vec::new();
    // The schema's shape is read once, at a process's first frame; what is measured is each frame's own cost.
    let heartbeat = proto::chatkord_v1::WireMessage {
        seq: 1,
        ack_of: None,
        body: Some(Body::Heartbeat(proto::chatkord_v1::Heartbeat { nonce: 1 })),
    };
    let _ = proto::envelope::read_frame(&proto::envelope::write_frame(&heartbeat).expect("a frame"));
    for op in every_op() {
        for field in tried(&op) {
            // The whole bound of empty entries, and just under as many as the entries bound lets through.
            for payload in [MAX_FRAME_BYTES - 16, 2 * (proto::envelope::MAX_FRAME_ENTRIES - 8)] {
                let bytes = frame(&op, field, payload);
                let spent = cost(&bytes);
                let name = format!("{} field {field}, {} bytes", op_name(&op), bytes.len());
                if spent > worst.0 {
                    worst = (spent, name.clone());
                }
                if spent > MOST_BYTES {
                    over.push(format!("{name}: {spent} bytes"));
                }
            }
        }
    }
    println!("the costliest frame decoded into {} bytes ({})", worst.0, worst.1);
    assert!(over.is_empty(), "{} frames decode into more than {MOST_BYTES} bytes:\n{}", over.len(), over.join("\n"));
}

fn op_name(op: &Body) -> String {
    format!("{op:?}").split('(').next().unwrap_or_default().to_string()
}

/// Each field the schema gives the op's message, and one number past them that it does not know.
fn tried(op: &Body) -> Vec<u32> {
    let mut fields = proto::envelope::fields_of(op);
    fields.push(fields.last().map_or(1, |last| last + 1));
    fields
}
