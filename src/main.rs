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

    /// Color theme name (e.g., Nord, Dracula, Gruvbox)
    #[arg(long, value_name = "NAME")]
    theme: Option<String>,

    /// List available themes and exit
    #[arg(long)]
    list_themes: bool,

    /// Skip session restore (start fresh)
    #[arg(long)]
    no_restore: bool,

    /// Search internet radio by name and play the first result
    #[arg(long, value_name = "QUERY")]
    radio: Option<String>,

    /// Fetch a podcast RSS feed and play the most recent episode
    #[arg(long, value_name = "URL")]
    podcast: Option<String>,

    /// List available audio output devices and exit
    #[arg(long)]
    list_devices: bool,

    /// Select audio output device by name (case-insensitive substring match)
    #[arg(long, value_name = "NAME")]
    device: Option<String>,
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

    if cli.list_themes {
        println!("Available themes:");
        for name in tui::theme::theme_names() {
            println!("  {name}");
        }
        return Ok(());
    }

    if cli.list_devices {
        let devices = backend::cpal_backend::list_devices()?;
        if devices.is_empty() {
            println!("No audio output devices found.");
        } else {
            println!("Available audio output devices:");
            for d in &devices {
                let marker = if d.is_default { " (default)" } else { "" };
                println!("  {}{marker}", d.name);
            }
        }
        return Ok(());
    }

    let volume = cli.volume.unwrap_or(config.default_volume);
    let eq_preset = cli.eq_preset.or(config.eq_preset);
    let theme_name = cli.theme.or_else(|| Some(config.theme.clone()));
    let rg_mode = playback::replaygain::ReplayGainMode::from_str_config(&config.replaygain);
    let device_name = cli.device.or(config.audio_device);

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
        let player = playback::player::Player::new(
            vec![track],
            volume,
            eq_preset.as_deref(),
            rg_mode,
            device_name.clone(),
        )?;
        tui::app::run_with_theme(player, cli.no_restore, theme_name.as_deref())?;
        return Ok(());
    }

    // Handle --podcast: fetch RSS feed and play the most recent episode.
    if let Some(ref feed_url) = cli.podcast {
        eprintln!("Fetching podcast feed: {feed_url}");
        let (feed, episodes) = providers::podcast::fetch_feed(feed_url)?;
        if episodes.is_empty() {
            eprintln!("No audio episodes found in \"{}\".", feed.title);
            std::process::exit(1);
        }
        eprintln!("Podcast: {}", feed.title);
        eprintln!("Episodes:");
        for (i, ep) in episodes.iter().enumerate() {
            let duration = ep
                .duration_secs
                .map(|s| format!(" ({}:{:02})", s / 60, s % 60))
                .unwrap_or_default();
            let date = ep.published.as_deref().unwrap_or("unknown date");
            eprintln!("  {}. {} [{}]{}", i + 1, ep.title, date, duration);
        }
        let track = providers::podcast::episode_to_track(&episodes[0]);
        eprintln!("Playing: {}", track.display_name());
        let player = playback::player::Player::new(
            vec![track],
            volume,
            eq_preset.as_deref(),
            rg_mode,
            device_name.clone(),
        )?;
        tui::app::run_with_theme(player, cli.no_restore, theme_name.as_deref())?;
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
    let player =
        playback::player::Player::new(tracks, volume, eq_preset.as_deref(), rg_mode, device_name)?;
    tui::app::run_with_theme(player, cli.no_restore, theme_name.as_deref())?;

    Ok(())
}
