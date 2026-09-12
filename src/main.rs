use std::process;

mod cli;
mod core;
mod driver;
mod error;
mod exec;

use crate::{core::Config, driver::konsole::KonsoleDriver};

fn main() {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();

    let args = cli::parse(&raw_args).unwrap_or_else(|usage| {
        eprint!("{usage}");
        process::exit(if raw_args.is_empty() { 1 } else { 0 });
    });

    let mut config = Config::parse_from_file(&args.config_path).unwrap_or_else(|err| {
        eprintln!("erro: {err}");
        process::exit(1);
    });

    if let Some(base) = args.base_override {
        config.workspace.base = base;
    }

    if let Err(err) = config.validate() {
        eprintln!("erro: {err}");
        process::exit(1);
    }

    match exec::run(&config, &KonsoleDriver) {
        Ok(summary) => {
            for pane in &summary.ran {
                println!("✓ {pane}");
            }
            for pane in &summary.failed {
                eprintln!("✗ {pane}");
            }
            if !summary.failed.is_empty() {
                process::exit(1);
            }
        }
        Err(err) => {
            eprintln!("erro: {err}");
            process::exit(1);
        }
    }
}
