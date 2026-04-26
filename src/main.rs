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

use crate::core::track::Track;
use crate::playback::stream_decoder;

#[derive(Parser)]
#[command(
    name = "kora",
    version,
    about = "A fast, multi-source terminal audio player"
)]
struct Cli {
    /// Files, directories, or URLs to play
    #[arg(value_name = "INPUT")]
    inputs: Vec<String>,

    /// Volume in dB (e.g., -3 for quieter, 0 for default). Overrides config.
    #[arg(long)]
    volume: Option<f32>,

    /// EQ preset name (e.g., Rock, Jazz, Pop, Classical, Bass Boost)
    #[arg(long, value_name = "PRESET")]
    eq_preset: Option<String>,

    /// List available EQ presets and exit
    #[arg(long)]
    list_eq_presets: bool,

    /// Skip session restore (start fresh)
    #[arg(long)]
    no_restore: bool,

    /// Search internet radio by name and play the first result
    #[arg(long, value_name = "QUERY")]
    radio: Option<String>,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let cli = Cli::parse();
    let config = core::config::KoraConfig::load()?;

    tracing::debug!(
        "Config path: {}",
        core::config::KoraConfig::config_path().display()
    );

    if cli.list_eq_presets {
        println!("Available EQ presets:");
        for name in playback::eq::preset_names() {
            let preset = playback::eq::find_preset(name).unwrap();
            let gains: Vec<String> = preset.gains.iter().map(|g| format!("{g:+.0}")).collect();
            println!("  {:<14} [{}]", name, gains.join(", "));
        }
        return Ok(());
    }

    let volume = cli.volume.unwrap_or(config.default_volume);
    let eq_preset = cli.eq_preset.or(config.eq_preset);

    // Handle --radio: search Radio Browser API and play the first match.
    if let Some(ref query) = cli.radio {
        eprintln!("Searching Radio Browser for \"{}\"...", query);
        let stations = providers::radio::search_by_name(query, 10)?;
        if stations.is_empty() {
            eprintln!("No stations found for \"{query}\".");
            std::process::exit(1);
        }
        eprintln!("Found {} station(s):", stations.len());
        for (i, s) in stations.iter().enumerate() {
            eprintln!(
                "  {}. {} [{} kbps, {}]",
                i + 1,
                s.name,
                s.bitrate,
                s.country
            );
        }
        let track = stations[0].to_track();
        eprintln!("Playing: {}", track.display_name());
        let player = playback::player::Player::new(vec![track], volume, eq_preset.as_deref())?;
        tui::app::run(player, cli.no_restore)?;
        return Ok(());
    }

    let inputs = if cli.inputs.is_empty() {
        if let Some(ref music_dir) = config.music_dir {
            vec![music_dir.to_string_lossy().into_owned()]
        } else {
            eprintln!("Usage: kora <file.mp3|file.flac|...>");
            eprintln!("       kora ~/Music/");
            eprintln!("       kora https://example.com/stream.mp3");
            eprintln!(
                "Tip: set music_dir in {} to skip this.",
                core::config::KoraConfig::config_path().display()
            );
            std::process::exit(1);
        }
    } else {
        cli.inputs
    };

    // Partition inputs into file paths and URLs
    let (url_inputs, file_inputs): (Vec<_>, Vec<_>) =
        inputs.iter().partition(|s| stream_decoder::is_url(s));

    let file_paths: Vec<PathBuf> = file_inputs.iter().map(PathBuf::from).collect();
    let mut tracks = providers::local::resolve_inputs(&file_paths)?;

    for url in &url_inputs {
        tracks.push(Track::from_url(url.to_string()));
    }

    if tracks.is_empty() {
        eprintln!("No playable audio files found.");
        std::process::exit(1);
    }

    tracing::info!("Playing {} track(s)", tracks.len());

    // Launch TUI player
    let player = playback::player::Player::new(tracks, volume, eq_preset.as_deref())?;
    tui::app::run(player, cli.no_restore)?;

    Ok(())
}
