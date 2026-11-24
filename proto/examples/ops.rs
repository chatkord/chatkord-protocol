//! Prints `gen/ops.rs` as the schema's options make it (`proto::ops::render`).

#[expect(clippy::print_stdout, reason = "the generator's output is its standard output")]
fn main() {
    print!("{}", proto::ops::render());
}
