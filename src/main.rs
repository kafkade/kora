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
mod ipc;
#[cfg(feature = "media-controls")]
mod media_controls;
mod playback;
mod providers;
mod tui;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use crate::core::track::Track;
use crate::playback::stream_decoder;

#[derive(Parser)]
#[command(
    name = "kora",
    version,
    about = "A fast, multi-source terminal audio player",
    args_conflicts_with_subcommands = true
)]
struct Cli {
    /// Remote control subcommand
    #[command(subcommand)]
    command: Option<CliCommand>,

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

    /// Import podcast subscriptions from an OPML file
    #[arg(long, value_name = "FILE")]
    import_opml: Option<String>,

    /// Export podcast subscriptions to an OPML file
    #[arg(long, value_name = "FILE")]
    export_opml: Option<String>,

    /// List available audio output devices and exit
    #[arg(long)]
    list_devices: bool,

    /// Select audio output device by name (case-insensitive substring match)
    #[arg(long, value_name = "NAME")]
    device: Option<String>,
}

#[derive(Subcommand)]
enum CliCommand {
    /// Send play command to running kora instance
    Play,
    /// Send pause command to running kora instance
    Pause,
    /// Toggle play/pause on running kora instance
    Toggle,
    /// Send stop command to running kora instance
    Stop,
    /// Skip to next track
    Next,
    /// Skip to previous track
    Prev,
    /// Set volume in dB (e.g., kora volume -- -3.0)
    Volume {
        /// Volume level in dB
        db: f32,
    },
    /// Get player status
    Status {
        /// Output status as JSON
        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let cli = Cli::parse();

    // Handle IPC subcommands (thin client mode)
    if let Some(cmd) = cli.command {
        return handle_ipc_command(cmd);
    }

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

    // Handle --import-opml: read OPML file and import feeds into podcast state.
    if let Some(ref opml_path) = cli.import_opml {
        let content = std::fs::read_to_string(opml_path)
            .with_context(|| format!("Failed to read OPML file: {opml_path}"))?;
        let entries = providers::opml::import_opml(&content)?;
        let total = entries.len();
        let mut state = providers::podcast::load_state()?;
        let added = state.import_feeds_from_opml(&entries);
        providers::podcast::save_state(&state)?;
        let skipped = total - added;
        println!("Imported {added} new feed(s) ({skipped} already subscribed)");
        return Ok(());
    }

    // Handle --export-opml: export podcast subscriptions to an OPML file.
    if let Some(ref opml_path) = cli.export_opml {
        let state = providers::podcast::load_state()?;
        let feeds = state.export_feeds();
        let xml = providers::opml::export_opml(feeds);
        std::fs::write(opml_path, &xml)
            .with_context(|| format!("Failed to write OPML file: {opml_path}"))?;
        println!("Exported {} feed(s) to {opml_path}", feeds.len());
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

fn handle_ipc_command(cmd: CliCommand) -> Result<()> {
    use crate::ipc::client::send_command;
    use crate::ipc::protocol::IpcRequest;

    let is_status = matches!(cmd, CliCommand::Status { .. });

    let request = match cmd {
        CliCommand::Play => IpcRequest::Play,
        CliCommand::Pause => IpcRequest::Pause,
        CliCommand::Toggle => IpcRequest::Toggle,
        CliCommand::Stop => IpcRequest::Stop,
        CliCommand::Next => IpcRequest::Next,
        CliCommand::Prev => IpcRequest::Prev,
        CliCommand::Volume { db } => IpcRequest::Volume { db },
        CliCommand::Status { .. } => IpcRequest::Status,
    };

    let response = send_command(&request)?;

    if is_status {
        println!(
            "{}",
            serde_json::to_string_pretty(&response).context("Failed to format response")?
        );
    } else if !response.ok {
        eprintln!(
            "Error: {}",
            response.error.as_deref().unwrap_or("unknown error")
        );
        std::process::exit(1);
    }

    Ok(())
}
