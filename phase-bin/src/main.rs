mod debug;

use std::path::PathBuf;

use phase::*;
use clap::Parser;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    bios: String,

    #[arg(short, long)]
    debug: bool,

    #[arg(short, long)]
    game: Option<String>,
}

fn main() {
    let args = Args::parse();

    let config = PlayStationConfig {
        bios_path: PathBuf::from(args.bios)
    };
    let playstation = PlayStation::new(&config);

    if args.debug {
        debug::debug_mode(playstation.make_debugger());
    } else {
        run(playstation);
    }
}

/// Run playstation with visuals.
fn run(mut playstation: PlayStation) {
    // TODO...
}