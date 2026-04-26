use std::io::{Read, Seek, SeekFrom};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use super::decoder::DecodedAudio;

const MAX_RETRIES: u32 = 3;
const RETRY_BACKOFF: Duration = Duration::from_secs(1);

/// Wrapper that implements `MediaSource` for a non-seekable HTTP response body.
struct HttpMediaSource {
    reader: Box<dyn Read + Send + Sync>,
    content_length: Option<u64>,
}

impl MediaSource for HttpMediaSource {
    fn is_seekable(&self) -> bool {
        false
    }

    fn byte_len(&self) -> Option<u64> {
        self.content_length
    }
}

impl Read for HttpMediaSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.reader.read(buf)
    }
}

impl Seek for HttpMediaSource {
    fn seek(&mut self, _pos: SeekFrom) -> std::io::Result<u64> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "HTTP streams are not seekable",
        ))
    }
}

/// Map an HTTP Content-Type header value to a symphonia file-extension hint.
pub fn content_type_to_hint(content_type: &str) -> Option<&'static str> {
    // Take only the MIME type, ignore parameters like charset
    let mime = content_type.split(';').next().unwrap_or("").trim();
    match mime {
        "audio/mpeg" | "audio/mp3" => Some("mp3"),
        "audio/flac" => Some("flac"),
        "audio/ogg" | "application/ogg" => Some("ogg"),
        "audio/wav" | "audio/x-wav" | "audio/wave" => Some("wav"),
        "audio/opus" => Some("opus"),
        "audio/aac" | "audio/x-aac" => Some("aac"),
        "audio/mp4" | "audio/x-m4a" | "audio/alac" => Some("m4a"),
        _ => None,
    }
}

/// Fetch and decode audio from an HTTP/HTTPS URL into f32 samples.
///
/// Retries the initial connection up to 3 times with 1-second backoff.
/// Once connected, the full response is streamed through symphonia's decoder.
pub fn decode_url(url: &str) -> Result<DecodedAudio> {
    let response = fetch_with_retry(url)?;

    let content_length = response.content_length();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();

    let mut hint = Hint::new();
    if let Some(ext) = content_type_to_hint(&content_type) {
        hint.with_extension(ext);
    } else {
        // Fall back to guessing from URL path
        if let Some(ext) = url_extension(url) {
            hint.with_extension(&ext);
        }
    }

    let source = HttpMediaSource {
        reader: Box::new(response),
        content_length,
    };
    let mss = MediaSourceStream::new(Box::new(source), Default::default());

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .with_context(|| format!("Failed to detect audio format from URL: {url}"))?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .with_context(|| format!("No supported audio track in URL: {url}"))?;

    let codec_params = track.codec_params.clone();
    let track_id = track.id;

    let sample_rate = codec_params
        .sample_rate
        .with_context(|| format!("Unknown sample rate in URL: {url}"))?;
    let channels = codec_params
        .channels
        .map(|c| c.count())
        .with_context(|| format!("Unknown channel count in URL: {url}"))?;

    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .with_context(|| format!("Failed to create decoder for URL: {url}"))?;

    let mut all_samples: Vec<f32> = Vec::new();
    let mut decode_errors = 0u32;
    let start = Instant::now();

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(symphonia::core::errors::Error::DecodeError(msg)) => {
                decode_errors += 1;
                tracing::warn!("Skipping corrupt packet from {url}: {msg}");
                continue;
            }
            Err(e) => return Err(e).context(format!("Read error streaming from {url}")),
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                let duration = decoded.capacity();
                let mut sample_buf = SampleBuffer::<f32>::new(duration as u64, spec);
                sample_buf.copy_interleaved_ref(decoded);
                all_samples.extend_from_slice(sample_buf.samples());
            }
            Err(symphonia::core::errors::Error::DecodeError(msg)) => {
                decode_errors += 1;
                tracing::warn!("Skipping corrupt frame from {url}: {msg}");
                continue;
            }
            Err(e) => return Err(e).context(format!("Decode error streaming from {url}")),
        }
    }

    let decode_time_ms = start.elapsed().as_secs_f64() * 1000.0;
    let audio_duration_s = all_samples.len() as f64 / (sample_rate as f64 * channels as f64);

    if all_samples.is_empty() {
        anyhow::bail!("No audio samples decoded from URL: {url}");
    }

    tracing::info!(
        "Decoded {:.1}s audio from URL in {:.0}ms ({:.0}x realtime, {}Hz, {}ch{})",
        audio_duration_s,
        decode_time_ms,
        (audio_duration_s * 1000.0) / decode_time_ms,
        sample_rate,
        channels,
        if decode_errors > 0 {
            format!(", {decode_errors} errors skipped")
        } else {
            String::new()
        }
    );

    Ok(DecodedAudio {
        samples: all_samples,
        sample_rate,
        channels,
        decode_time_ms,
    })
}

/// Fetch a URL with retry on connection failure.
fn fetch_with_retry(url: &str) -> Result<reqwest::blocking::Response> {
    let mut last_err = None;

    for attempt in 0..MAX_RETRIES {
        if attempt > 0 {
            tracing::info!(
                "Retrying URL (attempt {}/{}): {url}",
                attempt + 1,
                MAX_RETRIES
            );
            thread::sleep(RETRY_BACKOFF);
        }

        match reqwest::blocking::get(url) {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    return Ok(resp);
                }
                last_err = Some(anyhow::anyhow!("HTTP {status} fetching {url}"));
            }
            Err(e) => {
                last_err =
                    Some(anyhow::Error::new(e).context(format!("Connection failed for {url}")));
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("Failed to fetch {url}")))
}

/// Extract a file extension from a URL path, ignoring query parameters.
fn url_extension(url: &str) -> Option<String> {
    let path = url.split('?').next().unwrap_or(url);
    let filename = path.rsplit('/').next()?;
    let ext = filename.rsplit('.').next()?;
    if ext == filename {
        None
    } else {
        Some(ext.to_lowercase())
    }
}

/// Returns true if the input string looks like an HTTP/HTTPS URL.
pub fn is_url(input: &str) -> bool {
    input.starts_with("http://") || input.starts_with("https://")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_url_detects_http() {
        assert!(is_url("http://example.com/audio.mp3"));
    }

    #[test]
    fn is_url_detects_https() {
        assert!(is_url("https://example.com/stream"));
    }

    #[test]
    fn is_url_rejects_file_paths() {
        assert!(!is_url("/home/user/music.mp3"));
        assert!(!is_url("C:\\Music\\song.flac"));
        assert!(!is_url("relative/path.wav"));
        assert!(!is_url(""));
    }

    #[test]
    fn is_url_rejects_other_schemes() {
        assert!(!is_url("ftp://example.com/file.mp3"));
        assert!(!is_url("file:///home/user/music.mp3"));
    }

    #[test]
    fn content_type_maps_audio_mpeg_to_mp3() {
        assert_eq!(content_type_to_hint("audio/mpeg"), Some("mp3"));
    }

    #[test]
    fn content_type_maps_audio_mp3() {
        assert_eq!(content_type_to_hint("audio/mp3"), Some("mp3"));
    }

    #[test]
    fn content_type_maps_audio_flac() {
        assert_eq!(content_type_to_hint("audio/flac"), Some("flac"));
    }

    #[test]
    fn content_type_maps_audio_ogg() {
        assert_eq!(content_type_to_hint("audio/ogg"), Some("ogg"));
        assert_eq!(content_type_to_hint("application/ogg"), Some("ogg"));
    }

    #[test]
    fn content_type_maps_audio_wav() {
        assert_eq!(content_type_to_hint("audio/wav"), Some("wav"));
        assert_eq!(content_type_to_hint("audio/x-wav"), Some("wav"));
        assert_eq!(content_type_to_hint("audio/wave"), Some("wav"));
    }

    #[test]
    fn content_type_maps_audio_opus() {
        assert_eq!(content_type_to_hint("audio/opus"), Some("opus"));
    }

    #[test]
    fn content_type_maps_audio_aac() {
        assert_eq!(content_type_to_hint("audio/aac"), Some("aac"));
        assert_eq!(content_type_to_hint("audio/x-aac"), Some("aac"));
    }

    #[test]
    fn content_type_maps_audio_m4a() {
        assert_eq!(content_type_to_hint("audio/mp4"), Some("m4a"));
        assert_eq!(content_type_to_hint("audio/x-m4a"), Some("m4a"));
        assert_eq!(content_type_to_hint("audio/alac"), Some("m4a"));
    }

    #[test]
    fn content_type_ignores_parameters() {
        assert_eq!(
            content_type_to_hint("audio/mpeg; charset=utf-8"),
            Some("mp3")
        );
    }

    #[test]
    fn content_type_unknown_returns_none() {
        assert_eq!(content_type_to_hint("text/html"), None);
        assert_eq!(content_type_to_hint("application/json"), None);
        assert_eq!(content_type_to_hint(""), None);
    }

    #[test]
    fn url_extension_extracts_mp3() {
        assert_eq!(
            url_extension("https://example.com/song.mp3"),
            Some("mp3".to_string())
        );
    }

    #[test]
    fn url_extension_ignores_query_params() {
        assert_eq!(
            url_extension("https://example.com/song.flac?token=abc"),
            Some("flac".to_string())
        );
    }

    #[test]
    fn url_extension_returns_none_for_no_ext() {
        assert_eq!(url_extension("https://example.com/stream"), None);
    }

    #[test]
    fn url_extension_lowercases() {
        assert_eq!(
            url_extension("https://example.com/song.MP3"),
            Some("mp3".to_string())
        );
    }
}
