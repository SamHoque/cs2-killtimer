mod common;
mod core;
mod cs2;
mod hacks;
mod utils;

mod entry_point;

fn main() -> anyhow::Result<()> {
    entry_point::start()
}
