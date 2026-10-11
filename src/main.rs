mod cli;
mod color;
mod compose;
mod config;
mod decode;
mod descriptor;
mod grid;
mod index;
mod matcher;
mod media;
mod mosaic;
mod report;
mod validate;

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    match mosaic::run(&cli::Args::parse()) {
        Ok(_) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}
