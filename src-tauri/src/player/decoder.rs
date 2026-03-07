//! Audio decoding pipeline: channel → decode → rodio.
//!
//! Two decode paths:
//! 1. **Manual ADTS/AAC** — manual ADTS frame parsing + direct symphonia AAC codec
//! 2. **Symphonia probe** — for MP3 and other natively supported formats
//!
//! Format is auto-detected from the first bytes of the stream:
//! - ADTS sync (0xFFF, layer=00) with frame-chain validation → AAC path
//! - Anything else → symphonia probe path

use rodio::buffer::SamplesBuffer;
use rodio::Sink;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CodecParameters, DecoderOptions, CODEC_TYPE_AAC};
use symphonia::core::formats::Packet;
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use tauri::AppHandle;
use tracing::{debug, error, info, warn};

use crate::events;
use crate::player::eq::Equalizer;
use crate::player::types::{PlaybackStatus, PlayerMessage};

/// AAC sample rate table (ISO 14496-3)
const SAMPLE_RATES: [u32; 13] = [
    96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350,
];

pub fn run_decode_loop(
    audio_rx: Receiver<PlayerMessage>,
    sink: Arc<Sink>,
    shutdown: Arc<AtomicBool>,
    app_handle: AppHandle,
    emit_events: bool,
) {
    let mut reader = ChannelReader::new(audio_rx, Arc::clone(&shutdown));

    // Read initial bytes for format detection
    let mut peek = vec![0u8; 16384];
    let mut n = 0;
    while n < 16384 {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
        match reader.read(&mut peek[n..]) {
            Ok(0) => break, // EOF
            Ok(count) => n += count,
            Err(_) => break, // Error
        }
        // 8KB is plenty of data to find at least 3 consecutive ADTS frames (typically ~300 bytes each)
        if n >= 8192 {
            break;
        }
    }

    if n < 2 {
        error!("Failed to read initial stream bytes");
        if emit_events {
            events::emit_error(&app_handle, "No data from stream");
        }
        return;
    }
    peek.truncate(n);

    // ADTS detection with frame-chain validation.
    // When found, skip junk before the sync and use the manual ADTS decoder
    // which bypasses symphonia's probe entirely (the probe picks the MP3
    // reader over ADTS, which is the root cause of the "skipping junk" errors).
    let adts_offset = find_adts_sync(&peek);

    if let Some(offset) = adts_offset {
        if offset > 0 {
            info!("ADTS sync at offset {}, skipping leading bytes", offset);
            peek = peek[offset..].to_vec();
        }
        info!("Routing stream via manual ADTS decode");
        let prefixed = PrefixedReader::new(peek, reader);
        run_adts_decode(prefixed, sink, shutdown, app_handle, emit_events);
    } else {
        info!("Routing stream via symphonia probe");
        let prefixed = PrefixedReader::new(peek, reader);
        run_probe_decode(
            prefixed,
            Hint::new(),
            sink,
            shutdown,
            app_handle,
            emit_events,
        );
    }

    info!("Decode thread exiting");
}

// ===========================================================================
// ADTS detection
// ===========================================================================

/// Find the offset of a valid ADTS sync in the buffer.
/// Validates by checking the frame length leads to another valid ADTS sync
/// (frame-chain validation) to avoid false positives from MP3 sync words.
fn find_adts_sync(buf: &[u8]) -> Option<usize> {
    if buf.len() < 7 {
        return None;
    }
    for i in 0..buf.len().saturating_sub(7) {
        // ADTS sync: 0xFFF (12 bits), layer must be 00 (2 bits)
        if buf[i] != 0xFF || (buf[i + 1] & 0xF6) != 0xF0 {
            continue;
        }

        // Parse frame length (13 bits)
        let frame_len = ((buf[i + 3] as usize & 0x03) << 11)
            | ((buf[i + 4] as usize) << 3)
            | ((buf[i + 5] as usize) >> 5);

        if !(7..=8192).contains(&frame_len) {
            continue;
        }

        let next = i + frame_len;
        if next + 6 < buf.len() && buf[next] == 0xFF && (buf[next + 1] & 0xF6) == 0xF0 {
            // Read frame 2 length to find frame 3
            let frame_len_2 = ((buf[next + 3] as usize & 0x03) << 11)
                | ((buf[next + 4] as usize) << 3)
                | ((buf[next + 5] as usize) >> 5);

            let next_next = next + frame_len_2;
            if next_next + 1 < buf.len()
                && buf[next_next] == 0xFF
                && (buf[next_next + 1] & 0xF6) == 0xF0
            {
                return Some(i); // 3-frame chain confirmed
            }
        }
    }
    None
}

// ===========================================================================
// Audio level meter (lightweight, no IPC from decode thread)
// ===========================================================================

use std::sync::atomic::AtomicU32;

/// Shared audio level accessible from the frontend via command.
/// Stored as f32 bits in an AtomicU32 for lock-free access.
pub static AUDIO_LEVEL: AtomicU32 = AtomicU32::new(0);

/// Capture flag for song identification (Shazam-style)
pub static CAPTURE_ACTIVE: AtomicBool = AtomicBool::new(false);
/// Buffer for captured PCM samples (mono, 8000Hz preferred for recognition)
pub static CAPTURED_SAMPLES: Mutex<Vec<f32>> = Mutex::new(Vec::new());
/// Sample rate for the captured audio
pub static RECOGNITION_SAMPLE_RATE: AtomicU32 = AtomicU32::new(44100);

const LEVEL_SAMPLES: usize = 512;
const MAX_CAPTURE_SAMPLES: usize = 16000 * 12; // ~12 seconds at 16kHz

/// Lightweight audio level meter. Computes RMS from decoded PCM
/// and stores it in a global atomic — zero IPC overhead.
struct AudioLevelMeter {
    buffer: Vec<f32>,
    smoothed: f32,
}

impl AudioLevelMeter {
    fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(LEVEL_SAMPLES),
            smoothed: 0.0,
        }
    }

    fn feed(&mut self, samples: &[f32], channels: u16) {
        let ch = channels.max(1) as usize;

        // Handle audio identification capture
        if CAPTURE_ACTIVE.load(Ordering::Relaxed) {
            if let Ok(mut buffer) = CAPTURED_SAMPLES.lock() {
                if buffer.len() < MAX_CAPTURE_SAMPLES {
                    // Downsample/sum to mono for identification to save memory
                    for frame in samples.chunks(ch) {
                        let mono: f32 = frame.iter().sum::<f32>() / ch as f32;
                        buffer.push(mono);
                        if buffer.len() >= MAX_CAPTURE_SAMPLES {
                            break;
                        }
                    }
                }
            }
        }

        for frame in samples.chunks(ch) {
            let mono: f32 = frame.iter().sum::<f32>() / ch as f32;
            self.buffer.push(mono);
        }

        if self.buffer.len() >= LEVEL_SAMPLES {
            // Compute RMS
            let sum_sq: f32 = self.buffer.iter().map(|s| s * s).sum();
            let rms = (sum_sq / self.buffer.len() as f32).sqrt();

            // Log scale: map to 0.0..1.0
            let db = if rms > 1e-6 {
                (20.0 * rms.log10()).max(-48.0)
            } else {
                -48.0
            };
            let level = ((db + 48.0) / 48.0).clamp(0.0, 1.0);

            // Smooth
            if level > self.smoothed {
                self.smoothed = self.smoothed * 0.2 + level * 0.8;
            } else {
                self.smoothed = self.smoothed * 0.85 + level * 0.15;
            }

            // Store atomically — no lock, no IPC, no event
            AUDIO_LEVEL.store(self.smoothed.to_bits(), Ordering::Relaxed);
            self.buffer.clear();
        }
    }

    fn reset() {
        AUDIO_LEVEL.store(0u32, Ordering::Relaxed);
    }
}

// ===========================================================================
// Path 1: Manual ADTS/AAC decoder (bypasses symphonia probe)
// ===========================================================================

/// Manually parse ADTS frames and decode AAC via symphonia's codec directly.
/// This avoids symphonia's format probe which incorrectly picks the MP3 reader.
fn run_adts_decode(
    reader: PrefixedReader,
    sink: Arc<Sink>,
    shutdown: Arc<AtomicBool>,
    app_handle: AppHandle,
    emit_events: bool,
) {
    let mut reader = reader;
    let mut decoder: Option<Box<dyn symphonia::core::codecs::Decoder>> = None;
    let mut packet_ts: u64 = 0;
    let mut meter = AudioLevelMeter::new();
    let mut equalizer: Option<Equalizer> = None;

    if emit_events {
        events::emit_status(&app_handle, PlaybackStatus::Playing);
    }

    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        // Check for transition flush (RE-CHECK: Ensure we check at the START of the loop)
        if reader.flush_requested() {
            info!("ADTS Decoder: Transition detected! Flushing sink to skip buffered ads.");
            sink.pause();
            sink.clear();
            sink.play();
        }

        // Find ADTS sync word
        let mut sync = [0u8; 2];
        match reader.read_exact(&mut sync) {
            Ok(_) => {},
            Err(_) => break, // EOF or Error
        }

        // Resync if needed
        let mut resync_count = 0usize;
        while !(sync[0] == 0xFF && (sync[1] & 0xF6) == 0xF0) {
            sync[0] = sync[1];
            if reader.read_exact(&mut sync[1..2]).is_err() {
                return;
            }
            resync_count += 1;
            if resync_count > 65536 {
                warn!("ADTS: no sync after 64KB, giving up");
                if emit_events {
                    events::emit_error(&app_handle, "ADTS stream lost sync");
                }
                return;
            }
        }

        // Read remaining 5 bytes of the 7-byte ADTS fixed header
        let mut hdr_rest = [0u8; 5];
        if reader.read_exact(&mut hdr_rest).is_err() {
            break;
        }

        let header = [
            sync[0],
            sync[1],
            hdr_rest[0],
            hdr_rest[1],
            hdr_rest[2],
            hdr_rest[3],
            hdr_rest[4],
        ];

        let protection_absent = (header[1] & 0x01) != 0;
        let profile = ((header[2] >> 6) & 0x03) + 1; // MPEG-4 Audio Object Type
        let sample_rate_idx = ((header[2] >> 2) & 0x0F) as usize;
        let channel_config = ((header[2] & 0x01) << 2) | ((header[3] >> 6) & 0x03);
        let frame_length = (((header[3] & 0x03) as usize) << 11)
            | ((header[4] as usize) << 3)
            | ((header[5] >> 5) as usize);

        let header_size: usize = if protection_absent { 7 } else { 9 };

        if frame_length < header_size || frame_length > 8192 {
            continue; // Invalid frame, resync on next iteration
        }

        // Skip CRC bytes if present
        if !protection_absent {
            let mut crc = [0u8; 2];
            if reader.read_exact(&mut crc).is_err() {
                break;
            }
        }

        // Read AAC frame payload
        let payload_size = frame_length - header_size;
        let mut payload = vec![0u8; payload_size];
        if reader.read_exact(&mut payload).is_err() {
            break;
        }

        // Create AAC decoder on first valid frame
        if decoder.is_none() {
            let sample_rate = if sample_rate_idx < SAMPLE_RATES.len() {
                SAMPLE_RATES[sample_rate_idx]
            } else {
                44100
            };
            let channels: u16 = match channel_config {
                1 => 1,
                2 => 2,
                3 => 3,
                4 => 4,
                5 => 5,
                6 => 6,
                7 => 8,
                _ => 2,
            };

            // Build AudioSpecificConfig (ISO 14496-3 §1.6.2.1):
            // audioObjectType(5 bits) | samplingFrequencyIndex(4 bits) | channelConfiguration(4 bits)
            let aot = profile;
            let sri = sample_rate_idx as u8;
            let cc = channel_config;
            let asc: [u8; 2] = [(aot << 3) | (sri >> 1), ((sri & 1) << 7) | (cc << 3)];

            let mut codec_params = CodecParameters::new();
            codec_params
                .for_codec(CODEC_TYPE_AAC)
                .with_sample_rate(sample_rate);
            codec_params.extra_data = Some(Box::from(asc));

            match symphonia::default::get_codecs().make(&codec_params, &DecoderOptions::default()) {
                Ok(d) => {
                    info!(
                        "ADTS decode: AAC-LC profile={} {}ch {}Hz",
                        profile, channels, sample_rate
                    );
                    RECOGNITION_SAMPLE_RATE.store(sample_rate, Ordering::Relaxed);
                    decoder = Some(d);
                }
                Err(e) => {
                    error!("AAC codec creation failed: {}", e);
                    if emit_events {
                        events::emit_error(&app_handle, &format!("AAC codec failed: {}", e));
                    }
                    return;
                }
            }
        }

        // Decode the AAC frame
        let dec = decoder.as_mut().unwrap();
        let packet = Packet::new_from_boxed_slice(0, packet_ts, 1024, payload.into_boxed_slice());
        packet_ts += 1024; // AAC-LC = 1024 samples per frame

        match dec.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                let dur = decoded.capacity();
                let mut sb = SampleBuffer::<f32>::new(dur as u64, spec);
                sb.copy_interleaved_ref(decoded);
                let mut samples = sb.samples().to_vec();
                if !samples.is_empty() {
                    let ch = spec.channels.count() as u16;
                    let rate = spec.rate;

                    // 1. Wait for player capacity first.
                    // Keep a slightly larger buffer (8 frames, ~200ms) to prevent audio
                    // underruns and stuttering on Linux ALSA/PulseAudio.
                    while sink.len() > 8 {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                        if shutdown.load(Ordering::Relaxed) {
                            break;
                        }
                    }

                    // 2. ONLY THEN apply equalizer (to use the most current gains).
                    let eq = equalizer.get_or_insert_with(|| Equalizer::new(rate, ch));
                    eq.process(&mut samples);

                    meter.feed(&samples, ch);
                    sink.append(SamplesBuffer::new(ch, rate, samples));
                }
            }
            Err(symphonia::core::errors::Error::DecodeError(msg)) => {
                debug!("ADTS frame decode skip: {}", msg);
            }
            Err(e) => {
                warn!("ADTS fatal decode error: {}", e);
                break;
            }
        }
    }

    AudioLevelMeter::reset();
}

// ===========================================================================
// Path 2: Symphonia probe (MP3, OGG, etc.)
// ===========================================================================

fn run_probe_decode(
    reader: PrefixedReader,
    hint: Hint,
    sink: Arc<Sink>,
    shutdown: Arc<AtomicBool>,
    app_handle: AppHandle,
    emit_events: bool,
) {
    let mss = MediaSourceStream::new(Box::new(reader), Default::default());

    let probe_result = match symphonia::default::get_probe().format(
        &hint,
        mss,
        &symphonia::core::formats::FormatOptions::default(),
        &MetadataOptions::default(),
    ) {
        Ok(r) => r,
        Err(e) => {
            error!("Format probe failed: {}", e);
            if emit_events {
                events::emit_error(&app_handle, &format!("Format probe failed: {}", e));
            }
            return;
        }
    };

    let mut format = probe_result.format;
    let track = match format.default_track() {
        Some(t) => t.clone(),
        None => {
            if emit_events {
                events::emit_error(&app_handle, "No audio track found");
            }
            return;
        }
    };

    let channels = track
        .codec_params
        .channels
        .map(|c| c.count() as u16)
        .unwrap_or(2);
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let track_id = track.id;

    info!(
        "Probe decode: {}ch {}Hz track={}",
        channels, sample_rate, track_id
    );
    RECOGNITION_SAMPLE_RATE.store(sample_rate, Ordering::Relaxed);

    let mut decoder = match symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
    {
        Ok(d) => d,
        Err(e) => {
            if emit_events {
                events::emit_error(&app_handle, &format!("Codec failed: {}", e));
            }
            return;
        }
    };

    if emit_events {
        events::emit_status(&app_handle, PlaybackStatus::Playing);
    }

    let mut meter = AudioLevelMeter::new();
    let mut equalizer = Equalizer::new(sample_rate, channels);

    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => {
                warn!("Packet error: {}", e);
                continue;
            }
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                let dur = decoded.capacity();
                let mut sb = SampleBuffer::<f32>::new(dur as u64, spec);
                sb.copy_interleaved_ref(decoded);
                let mut samples = sb.samples().to_vec();
                if !samples.is_empty() {
                    // Wait for player capacity first, allowing ~8 frames of buffering
                    // to prevent audio stuttering on Linux.
                    while sink.len() > 8 {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                        if shutdown.load(Ordering::Relaxed) {
                            break;
                        }
                    }

                    // Apply EQ just before sending to speaker.
                    equalizer.process(&mut samples);
                    meter.feed(&samples, channels);

                    sink.append(SamplesBuffer::new(channels, sample_rate, samples));
                }
            }
            Err(symphonia::core::errors::Error::DecodeError(msg)) => {
                debug!("Decode skip: {}", msg);
            }
            Err(e) => {
                warn!("Fatal decode error: {}", e);
                break;
            }
        }
    }

    AudioLevelMeter::reset();
}

// ===========================================================================
// I/O adapters
// ===========================================================================

/// Bridges `mpsc::Receiver<Bytes>` → `Read + Seek + MediaSource` for symphonia.
struct ChannelReader {
    rx: Mutex<Receiver<PlayerMessage>>,
    buffer: Vec<u8>,
    pos: usize,
    shutdown: Arc<AtomicBool>,
    flush_requested: Arc<AtomicBool>,
}

impl ChannelReader {
    fn new(rx: Receiver<PlayerMessage>, shutdown: Arc<AtomicBool>) -> Self {
        Self {
            rx: Mutex::new(rx),
            buffer: Vec::new(),
            pos: 0,
            shutdown,
            flush_requested: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Read for ChannelReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos < self.buffer.len() {
            let n = std::cmp::min(buf.len(), self.buffer.len() - self.pos);
            buf[..n].copy_from_slice(&self.buffer[self.pos..self.pos + n]);
            self.pos += n;
            return Ok(n);
        }
        if self.shutdown.load(Ordering::Relaxed) {
            return Ok(0);
        }

        // Loop until we get audio or EOF, handling Flush internally
        loop {
            let rx = self.rx.lock().unwrap();
            match rx.recv() {
                Ok(msg) => {
                    drop(rx);
                    match msg {
                        PlayerMessage::Audio(bytes) => {
                            self.buffer = bytes.to_vec();
                            self.pos = 0;
                            let n = std::cmp::min(buf.len(), self.buffer.len());
                            buf[..n].copy_from_slice(&self.buffer[..n]);
                            self.pos = n;
                            return Ok(n);
                        }
                        PlayerMessage::Flush => {
                            info!("ChannelReader: FLUSH signal received, marking for decoder");
                            self.flush_requested.store(true, Ordering::Relaxed);
                            // Do NOT return here, continue loop to get actual audio data
                            continue;
                        }
                    }
                }
                Err(_) => return Ok(0),
            }
        }
    }
}

impl Seek for ChannelReader {
    fn seek(&mut self, _: SeekFrom) -> io::Result<u64> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "not seekable"))
    }
}

impl MediaSource for ChannelReader {
    fn is_seekable(&self) -> bool {
        false
    }
    fn byte_len(&self) -> Option<u64> {
        None
    }
}

/// Prepends buffered bytes before delegating to inner reader.
/// Used to replay peeked bytes through the decode pipeline.
struct PrefixedReader {
    prefix: Vec<u8>,
    prefix_pos: usize,
    inner: ChannelReader,
}

impl PrefixedReader {
    fn new(prefix: Vec<u8>, inner: ChannelReader) -> Self {
        Self {
            prefix,
            prefix_pos: 0,
            inner,
        }
    }

    fn flush_requested(&mut self) -> bool {
        self.inner.flush_requested.swap(false, Ordering::Relaxed)
    }
}

impl Read for PrefixedReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.prefix_pos < self.prefix.len() {
            let n = std::cmp::min(buf.len(), self.prefix.len() - self.prefix_pos);
            buf[..n].copy_from_slice(&self.prefix[self.prefix_pos..self.prefix_pos + n]);
            self.prefix_pos += n;
            Ok(n)
        } else {
            self.inner.read(buf)
        }
    }
}

impl Seek for PrefixedReader {
    fn seek(&mut self, _: SeekFrom) -> io::Result<u64> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "not seekable"))
    }
}

impl MediaSource for PrefixedReader {
    fn is_seekable(&self) -> bool {
        false
    }
    fn byte_len(&self) -> Option<u64> {
        None
    }
}
