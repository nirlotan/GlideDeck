//! Video playback.
//!
//! Decoding is delegated to a locally installed FFmpeg executable (run as a
//! subprocess, never downloaded or contacted over the network): one process
//! streams raw RGBA frames, another streams PCM audio into the system output
//! through cpal. This keeps GlideDeck pure Rust with no C library linking,
//! while supporting every format FFmpeg does.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Longest side of decoded frames; plenty for on-canvas playback.
const MAX_DECODE_SIDE: u32 = 1280;

#[derive(Clone, Debug, PartialEq)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration: f64,
    pub has_audio: bool,
}

fn tool(name: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) { format!("{name}.exe") } else { name.to_owned() };
    if let Some(p) = std::env::var_os(format!("GLIDEDECK_{}", name.to_uppercase())) {
        return Some(PathBuf::from(p));
    }
    // Next to the GlideDeck executable (e.g. bundled by an installer).
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        let p = dir.join(&exe);
        if p.exists() {
            return Some(p);
        }
    }
    let path = std::env::var_os("PATH")?;
    let mut dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
    if cfg!(target_os = "macos") {
        dirs.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
    }
    dirs.into_iter().map(|d| d.join(&exe)).find(|p| p.exists())
}

/// Location of the `ffmpeg` executable, if installed.
pub fn ffmpeg_path() -> Option<PathBuf> {
    tool("ffmpeg")
}

fn command(program: &Path) -> Command {
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c.stdin(Stdio::null()).stderr(Stdio::null());
    c
}

/// Reads stream information with ffprobe.
pub fn probe(file: &Path) -> Result<VideoInfo, String> {
    let ffprobe = tool("ffprobe").ok_or("FFmpeg (ffprobe) is not installed")?;
    let out = command(&ffprobe)
        .args(["-v", "error", "-print_format", "json", "-show_streams", "-show_format"])
        .arg(file)
        .output()
        .map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let streams = v["streams"].as_array().ok_or("no streams")?;
    let video = streams.iter().find(|s| s["codec_type"] == "video").ok_or("no video stream")?;
    let ratio = |s: &str| -> Option<f64> {
        let (a, b) = s.split_once('/')?;
        let (a, b) = (a.parse::<f64>().ok()?, b.parse::<f64>().ok()?);
        (b > 0.0 && a > 0.0).then_some(a / b)
    };
    let fps = video["avg_frame_rate"].as_str().and_then(ratio).or_else(|| video["r_frame_rate"].as_str().and_then(ratio)).unwrap_or(30.0);
    let mut width = video["width"].as_u64().unwrap_or(0) as u32;
    let mut height = video["height"].as_u64().unwrap_or(0) as u32;
    // Honour rotation metadata (phone videos).
    let rotation = video["side_data_list"]
        .as_array()
        .and_then(|l| l.iter().find_map(|d| d["rotation"].as_f64()))
        .or_else(|| video["tags"]["rotate"].as_str().and_then(|r| r.parse().ok()))
        .unwrap_or(0.0);
    if (rotation.abs() - 90.0).abs() < 1.0 || (rotation.abs() - 270.0).abs() < 1.0 {
        std::mem::swap(&mut width, &mut height);
    }
    let duration = v["format"]["duration"].as_str().and_then(|d| d.parse().ok()).unwrap_or(0.0);
    let has_audio = streams.iter().any(|s| s["codec_type"] == "audio");
    if width == 0 || height == 0 {
        return Err("could not read the video size".into());
    }
    Ok(VideoInfo { width, height, fps: fps.clamp(1.0, 120.0), duration, has_audio })
}

fn decode_size(info: &VideoInfo) -> (u32, u32) {
    let k = (MAX_DECODE_SIDE as f64 / info.width.max(info.height) as f64).min(1.0);
    // Even dimensions keep every pixel format happy.
    let even = |v: f64| ((v.round() as u32).max(2) / 2) * 2;
    (even(info.width as f64 * k), even(info.height as f64 * k))
}

/// One RGBA frame at `seconds` (the poster image).
pub fn frame_at(file: &Path, info: &VideoInfo, seconds: f64) -> Option<(u32, u32, Vec<u8>)> {
    let ffmpeg = ffmpeg_path()?;
    let (w, h) = decode_size(info);
    let out = command(&ffmpeg)
        .args(["-v", "error", "-ss", &format!("{seconds:.3}"), "-i"])
        .arg(file)
        .args(["-frames:v", "1", "-an", "-f", "rawvideo", "-pix_fmt", "rgba", "-vf", &format!("scale={w}:{h}"), "-"])
        .stdout(Stdio::piped())
        .output()
        .ok()?;
    (out.stdout.len() == (w * h * 4) as usize).then_some((w, h, out.stdout))
}

/// A PNG poster frame (for exports).
pub fn poster_png(file: &Path) -> Option<Vec<u8>> {
    let info = probe(file).ok()?;
    let at = (info.duration * 0.1).min(1.0);
    let (w, h, rgba) = frame_at(file, &info, at)?;
    let img = image::RgbaImage::from_raw(w, h, rgba)?;
    let mut png = std::io::Cursor::new(Vec::new());
    img.write_to(&mut png, image::ImageFormat::Png).ok()?;
    Some(png.into_inner())
}

struct Shared {
    playing: AtomicBool,
    stop: AtomicBool,
    looping: AtomicBool,
    muted: AtomicBool,
    ended: AtomicBool,
    /// Bumped to restart decoding from the beginning.
    generation: AtomicU64,
    position_ms: AtomicU64,
    frame: Mutex<(u64, Vec<u8>)>,
    audio: Mutex<VecDeque<f32>>,
}

pub struct Player {
    pub info: VideoInfo,
    pub size: (u32, u32),
    shared: Arc<Shared>,
    _stream: Option<cpal::Stream>,
    shown_seq: u64,
}

impl Player {
    pub fn new(file: &Path, info: VideoInfo) -> Option<Player> {
        let ffmpeg = ffmpeg_path()?;
        let size = decode_size(&info);
        let shared = Arc::new(Shared {
            playing: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            looping: AtomicBool::new(false),
            muted: AtomicBool::new(false),
            ended: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            position_ms: AtomicU64::new(0),
            frame: Mutex::new((0, Vec::new())),
            audio: Mutex::new(VecDeque::new()),
        });
        {
            let (shared, file, ffmpeg, info) = (shared.clone(), file.to_path_buf(), ffmpeg.clone(), info.clone());
            std::thread::spawn(move || video_thread(&ffmpeg, &file, &info, size, &shared));
        }
        let stream = if info.has_audio { audio_stream(&ffmpeg, file, &shared) } else { None };
        Some(Player { info, size, shared, _stream: stream, shown_seq: 0 })
    }

    pub fn is_playing(&self) -> bool {
        self.shared.playing.load(Ordering::Relaxed)
    }

    pub fn play(&self) {
        if self.shared.ended.swap(false, Ordering::Relaxed) {
            self.restart();
        }
        self.shared.playing.store(true, Ordering::Relaxed);
    }

    pub fn pause(&self) {
        self.shared.playing.store(false, Ordering::Relaxed);
    }

    pub fn restart(&self) {
        self.shared.generation.fetch_add(1, Ordering::Relaxed);
        self.shared.audio.lock().map(|mut a| a.clear()).ok();
    }

    pub fn set_looping(&self, v: bool) {
        self.shared.looping.store(v, Ordering::Relaxed);
    }

    pub fn set_muted(&self, v: bool) {
        self.shared.muted.store(v, Ordering::Relaxed);
    }

    pub fn position(&self) -> f64 {
        self.shared.position_ms.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// A frame newer than the last one returned, if any.
    pub fn take_frame(&mut self) -> Option<Vec<u8>> {
        let f = self.shared.frame.lock().ok()?;
        if f.0 > self.shown_seq && !f.1.is_empty() {
            self.shown_seq = f.0;
            Some(f.1.clone())
        } else {
            None
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
    }
}

fn spawn_video(ffmpeg: &Path, file: &Path, (w, h): (u32, u32)) -> Option<Child> {
    command(ffmpeg)
        .args(["-v", "error", "-i"])
        .arg(file)
        .args(["-an", "-f", "rawvideo", "-pix_fmt", "rgba", "-vf", &format!("scale={w}:{h}"), "-"])
        .stdout(Stdio::piped())
        .spawn()
        .ok()
}

fn video_thread(ffmpeg: &Path, file: &Path, info: &VideoInfo, size: (u32, u32), s: &Shared) {
    let frame_len = (size.0 * size.1 * 4) as usize;
    let mut seq = 0u64;
    'outer: loop {
        let generation = s.generation.load(Ordering::Relaxed);
        let Some(mut child) = spawn_video(ffmpeg, file, size) else { return };
        let mut out = child.stdout.take().expect("piped stdout");
        let mut buf = vec![0u8; frame_len];
        let (mut n, mut clock, mut last) = (0u64, Duration::ZERO, Instant::now());
        loop {
            // Paused: hold the pipe (ffmpeg blocks) and stop the clock.
            while !s.playing.load(Ordering::Relaxed) && n > 0 {
                if s.stop.load(Ordering::Relaxed) || s.generation.load(Ordering::Relaxed) != generation {
                    let _ = child.kill();
                    if s.stop.load(Ordering::Relaxed) {
                        return;
                    }
                    continue 'outer;
                }
                std::thread::sleep(Duration::from_millis(15));
                last = Instant::now();
            }
            if s.stop.load(Ordering::Relaxed) {
                let _ = child.kill();
                return;
            }
            if s.generation.load(Ordering::Relaxed) != generation {
                let _ = child.kill();
                continue 'outer;
            }
            if out.read_exact(&mut buf).is_err() {
                break;
            }
            // Pace to the frame rate (the first frame is shown immediately as the poster).
            let target = Duration::from_secs_f64(n as f64 / info.fps);
            if n > 0 {
                loop {
                    let now = Instant::now();
                    if s.playing.load(Ordering::Relaxed) {
                        clock += now - last;
                    }
                    last = now;
                    if clock >= target || s.stop.load(Ordering::Relaxed) || s.generation.load(Ordering::Relaxed) != generation {
                        break;
                    }
                    std::thread::sleep((target - clock).min(Duration::from_millis(8)));
                }
            }
            n += 1;
            seq += 1;
            if let Ok(mut f) = s.frame.lock() {
                *f = (seq, buf.clone());
            }
            s.position_ms.store((target.as_secs_f64() * 1000.0) as u64, Ordering::Relaxed);
        }
        let _ = child.kill();
        // End of stream.
        if s.looping.load(Ordering::Relaxed) && s.playing.load(Ordering::Relaxed) {
            s.generation.fetch_add(1, Ordering::Relaxed);
            continue;
        }
        s.playing.store(false, Ordering::Relaxed);
        s.ended.store(true, Ordering::Relaxed);
        // Wait for a restart (play after the end, or an explicit restart).
        while s.generation.load(Ordering::Relaxed) == generation {
            if s.stop.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    }
}

/// Starts an audio output stream fed by an ffmpeg PCM decoder.
fn audio_stream(ffmpeg: &Path, file: &Path, shared: &Arc<Shared>) -> Option<cpal::Stream> {
    let device = cpal::default_host().default_output_device()?;
    let supported = device.default_output_config().ok()?;
    let config = supported.config();
    let (rate, channels) = (config.sample_rate, config.channels as usize);

    {
        let (shared, file, ffmpeg) = (shared.clone(), file.to_path_buf(), ffmpeg.to_path_buf());
        std::thread::spawn(move || audio_thread(&ffmpeg, &file, rate, channels, &shared));
    }
    let s = shared.clone();
    let err = |e| log::warn!("audio stream error: {e}");
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream::<f32, _, _>(config, move |out, _| fill(out, &s), err, None),
        cpal::SampleFormat::I16 => device.build_output_stream::<i16, _, _>(config, move |out, _| fill(out, &s), err, None),
        cpal::SampleFormat::U16 => device.build_output_stream::<u16, _, _>(config, move |out, _| fill(out, &s), err, None),
        _ => return None,
    }
    .ok()?;
    stream.play().ok()?;
    Some(stream)
}

fn fill<T: cpal::SizedSample + cpal::FromSample<f32>>(out: &mut [T], s: &Shared) {
    let playing = s.playing.load(Ordering::Relaxed);
    let muted = s.muted.load(Ordering::Relaxed);
    let Ok(mut q) = s.audio.try_lock() else {
        out.iter_mut().for_each(|o| *o = T::from_sample(0.0));
        return;
    };
    for o in out.iter_mut() {
        let v = if playing { q.pop_front().unwrap_or(0.0) } else { 0.0 };
        *o = T::from_sample(if muted { 0.0 } else { v });
    }
}

fn audio_thread(ffmpeg: &Path, file: &Path, rate: u32, channels: usize, s: &Shared) {
    let max = rate as usize * channels / 3; // ~330 ms of buffered audio
    'outer: loop {
        let generation = s.generation.load(Ordering::Relaxed);
        let Ok(mut child) = command(ffmpeg)
            .args(["-v", "error", "-i"])
            .arg(file)
            .args(["-vn", "-f", "f32le", "-ac", &channels.to_string(), "-ar", &rate.to_string(), "-"])
            .stdout(Stdio::piped())
            .spawn()
        else {
            return;
        };
        let mut out = child.stdout.take().expect("piped stdout");
        let mut chunk = vec![0u8; 4096];
        loop {
            if s.stop.load(Ordering::Relaxed) {
                let _ = child.kill();
                return;
            }
            if s.generation.load(Ordering::Relaxed) != generation {
                let _ = child.kill();
                continue 'outer;
            }
            let full = s.audio.lock().map(|q| q.len() >= max).unwrap_or(true);
            if full {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            match out.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let n = n - n % 4;
                    if let Ok(mut q) = s.audio.lock() {
                        q.extend(chunk[..n].chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])));
                    }
                }
            }
        }
        let _ = child.kill();
        while s.generation.load(Ordering::Relaxed) == generation {
            if s.stop.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    }
}
