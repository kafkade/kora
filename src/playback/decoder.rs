use std::fs::File;
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Decoded audio data ready for the audio pipeline.
#[derive(Debug)]
pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: usize,
    pub decode_time_ms: f64,
}

/// Decode an entire audio file into f32 samples.
///
/// Handles corrupt and truncated files gracefully: decode errors on individual
/// packets are logged and skipped rather than propagating as fatal errors.
/// Only a complete failure to open or probe the file is fatal.
pub fn decode_file(path: &Path) -> Result<DecodedAudio> {
    let file = File::open(path).with_context(|| format!("Cannot open {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .with_context(|| format!("Unsupported or corrupt audio file: {}", path.display()))?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .with_context(|| format!("No supported audio track in {}", path.display()))?;

    let codec_params = track.codec_params.clone();
    let track_id = track.id;

    let sample_rate = codec_params
        .sample_rate
        .with_context(|| format!("Unknown sample rate in {}", path.display()))?;
    let channels = codec_params
        .channels
        .map(|c| c.count())
        .with_context(|| format!("Unknown channel count in {}", path.display()))?;

    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .with_context(|| format!("Failed to create decoder for {}", path.display()))?;

    let mut all_samples: Vec<f32> = Vec::new();
    let mut decode_errors = 0u32;
    let start = Instant::now();

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break; // End of stream
            }
            Err(symphonia::core::errors::Error::DecodeError(msg)) => {
                // Corrupt packet — skip it, don't crash
                decode_errors += 1;
                tracing::warn!("Skipping corrupt packet in {}: {msg}", path.display());
                continue;
            }
            Err(e) => return Err(e).context(format!("Read error in {}", path.display())),
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
                // Corrupt frame — skip it, continue decoding
                decode_errors += 1;
                tracing::warn!("Skipping corrupt frame in {}: {msg}", path.display());
                continue;
            }
            Err(e) => return Err(e).context(format!("Decode error in {}", path.display())),
        }
    }

    let decode_time = start.elapsed();
    let decode_time_ms = decode_time.as_secs_f64() * 1000.0;
    let audio_duration_s = all_samples.len() as f64 / (sample_rate as f64 * channels as f64);

    if all_samples.is_empty() {
        anyhow::bail!("No audio samples decoded from {}", path.display());
    }

    tracing::info!(
        "Decoded {:.1}s audio in {:.0}ms ({:.0}x realtime, {}Hz, {}ch{})",
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn decode_nonexistent_file_returns_error() {
        let result = decode_file(Path::new("/nonexistent/file.mp3"));
        assert!(result.is_err());
        let err = format!("{:#}", result.unwrap_err());
        assert!(
            err.contains("Cannot open"),
            "Expected 'Cannot open' in error: {err}"
        );
    }

    #[test]
    fn decode_empty_file_returns_error() {
        let dir = std::env::temp_dir().join("kora_test_decode");
        std::fs::create_dir_all(&dir).unwrap();
        let empty_file = dir.join("empty.mp3");
        File::create(&empty_file).unwrap();

        let result = decode_file(&empty_file);
        assert!(result.is_err(), "Empty file should fail to decode");

        std::fs::remove_file(&empty_file).ok();
    }

    #[test]
    fn decode_garbage_file_returns_error() {
        let dir = std::env::temp_dir().join("kora_test_decode");
        std::fs::create_dir_all(&dir).unwrap();
        let garbage_file = dir.join("garbage.mp3");
        let mut f = File::create(&garbage_file).unwrap();
        f.write_all(&[0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01, 0x02, 0x03])
            .unwrap();

        let result = decode_file(&garbage_file);
        assert!(result.is_err(), "Garbage file should fail to decode");

        std::fs::remove_file(&garbage_file).ok();
    }

    #[test]
    fn decode_unsupported_extension_returns_error() {
        let dir = std::env::temp_dir().join("kora_test_decode");
        std::fs::create_dir_all(&dir).unwrap();
        let txt_file = dir.join("notaudio.txt");
        let mut f = File::create(&txt_file).unwrap();
        f.write_all(b"this is not audio").unwrap();

        let result = decode_file(&txt_file);
        assert!(result.is_err(), "Text file should fail to decode");

        std::fs::remove_file(&txt_file).ok();
    }
}
