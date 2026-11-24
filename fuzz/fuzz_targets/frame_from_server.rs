//! A frame from a server the client connected to, which may be anyone's, into the client's reader.
#![no_main]
#![forbid(unsafe_code)]

libfuzzer_sys::fuzz_target!(|data: &[u8]| proto::fuzz::frame_from_server(data));
