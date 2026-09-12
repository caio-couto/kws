use std::process;

mod core;
mod error;
mod exec;

use crate::core::Config;

fn main() {
    let config = Config::load_from_file("./kws.toml").unwrap_or_else(|err| {
        eprintln!("erro: {err}");
        process::exit(1);
    });

    println!("{config:?}");
}
