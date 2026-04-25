// kora — a fast, multi-source terminal audio player
//
// Module structure follows the layered architecture:
//   core/     — domain models, traits, config (no audio deps)
//   playback/ — decode, DSP, state machine (symphonia, rtrb)
//   backend/  — audio output adapters (cpal)
//   providers/ — audio source implementations (local, radio, podcast)
//   tui/      — terminal UI (ratatui)
//   ipc/      — remote control protocol

mod backend;
mod core;
mod playback;
mod providers;
mod tui;

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
#[command(
    name = "kora",
    version,
    about = "A fast, multi-source terminal audio player"
)]
struct Cli {
    /// Files, directories, or URLs to play
    #[arg(value_name = "INPUT")]
    inputs: Vec<PathBuf>,

    /// Volume in dB (e.g., -3 for quieter, 0 for default)
    #[arg(long, default_value = "0")]
    volume: f32,

    /// EQ preset name (e.g., Rock, Jazz, Pop, Classical, Bass Boost)
    #[arg(long, value_name = "PRESET")]
    eq_preset: Option<String>,

    /// List available EQ presets and exit
    #[arg(long)]
    list_eq_presets: bool,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let cli = Cli::parse();

    if cli.list_eq_presets {
        println!("Available EQ presets:");
        for name in playback::eq::preset_names() {
            let preset = playback::eq::find_preset(name).unwrap();
            let gains: Vec<String> = preset.gains.iter().map(|g| format!("{g:+.0}")).collect();
            println!("  {:<14} [{}]", name, gains.join(", "));
        }
        return Ok(());
    }

    if cli.inputs.is_empty() {
        eprintln!("Usage: kora <file.mp3|file.flac|...>");
        eprintln!("       kora ~/Music/");
        std::process::exit(1);
    }

    let tracks = providers::local::resolve_inputs(&cli.inputs)?;

    if tracks.is_empty() {
        eprintln!("No playable audio files found.");
        std::process::exit(1);
    }

    tracing::info!("Playing {} track(s)", tracks.len());

    // Launch TUI player
    let player = playback::player::Player::new(tracks, cli.volume, cli.eq_preset.as_deref())?;
    tui::app::run(player)?;

    Ok(())
}
