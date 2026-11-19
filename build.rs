//! The daemon embeds `web/dist` at compile time (rust-embed), but nothing tells cargo to watch it,
//! so a rebuilt web UI alone would leave the binary serving the old one. Watch it here.
fn main() {
    println!("cargo:rerun-if-changed=web/dist");
}
