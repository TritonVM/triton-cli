use clap::Parser;

use crate::args::Command;

mod args;

fn main() {
    match Command::parse() {
        Command::Run(..) => {}
        Command::Prove { .. } => {}
        Command::Verify(..) => {}
    }
}
