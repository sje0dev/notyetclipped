use std::{
    collections::VecDeque,
    path::{Component, Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use anyhow::{Context, Result, ensure};
use tokio::process::Command;

use crate::{config::Config, models::ManifestEntry};

pub fn safe_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let relative = Path::new(relative);
    ensure!(
        !relative.is_absolute()
            && relative
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        "Ungültiger Dateipfad."
    );
    let path = root.join(relative);
    // Check every existing ancestor so a symlink cannot escape the scratch root.
    let mut current = path.as_path();
    while !current.exists() {
        current = current.parent().context("Ungültiger Dateipfad")?;
    }
    ensure!(
        current.canonicalize()?.starts_with(root.canonicalize()?),
        "Dateipfad außerhalb des Arbeitsverzeichnisses."
    );
    Ok(path)
}

pub async fn ffmpeg(config: &Config, args: &[String], timeout: Duration) -> Result<Vec<u8>> {
    let output = tokio::time::timeout(
        timeout,
        Command::new(&config.ffmpeg)
            .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y"])
            .args(args)
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .context("FFmpeg-Zeitlimit erreicht.")??;
    ensure!(
        output.status.success(),
        "FFmpeg: {}",
        tail(&String::from_utf8_lossy(&output.stderr), 1000)
    );
    Ok(output.stdout)
}

pub fn tail(text: &str, length: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    chars[chars.len().saturating_sub(length)..].iter().collect()
}

pub async fn decode_audio(config: &Config, path: &Path) -> Result<Vec<i16>> {
    let args = [
        "-i",
        &path.to_string_lossy(),
        "-vn",
        "-ac",
        "1",
        "-ar",
        "16000",
        "-t",
        "20",
        "-f",
        "s16le",
        "pipe:1",
    ]
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();
    Ok(ffmpeg(config, &args, Duration::from_secs(30))
        .await?
        .as_chunks::<2>()
        .0
        .iter()
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect())
}

pub async fn render(
    config: &Config,
    manifest: &[ManifestEntry],
    start: i64,
    end: i64,
    directory: &Path,
) -> Result<(PathBuf, PathBuf)> {
    tokio::fs::create_dir_all(directory).await?;
    let video = directory.join("video.mp4");
    let thumbnail = directory.join("thumbnail.jpg");
    if !video.is_file() {
        ensure!(!manifest.is_empty(), "Keine Videosegmente verfügbar.");
        let mut concat = String::from("ffconcat version 1.0\n");
        for entry in manifest {
            let path = safe_path(&config.data_dir, &entry.path)?;
            ensure!(
                path.is_file(),
                "Benötigte Videosegmente sind nicht mehr im Puffer."
            );
            concat.push_str(&format!(
                "file '{}'\n",
                path.to_string_lossy().replace('\'', "'\\''")
            ));
        }
        let input = directory.join("input.ffconcat");
        tokio::fs::write(&input, concat).await?;
        let temporary = directory.join("video.partial.mp4");
        let args = vec![
            "-f".into(),
            "concat".into(),
            "-safe".into(),
            "0".into(),
            "-i".into(),
            input.to_string_lossy().into(),
            "-ss".into(),
            ((start - manifest[0].start_ms).max(0) as f64 / 1000.0).to_string(),
            "-t".into(),
            ((end - start) as f64 / 1000.0).to_string(),
            "-map".into(),
            "0:v:0".into(),
            "-map".into(),
            "0:a:0?".into(),
            "-c:v".into(),
            "libx264".into(),
            "-threads".into(),
            "2".into(),
            "-preset".into(),
            "veryfast".into(),
            "-crf".into(),
            "23".into(),
            "-pix_fmt".into(),
            "yuv420p".into(),
            "-c:a".into(),
            "aac".into(),
            "-b:a".into(),
            "128k".into(),
            "-movflags".into(),
            "+faststart".into(),
            temporary.to_string_lossy().into(),
        ];
        ffmpeg(config, &args, Duration::from_secs(600)).await?;
        ensure!(
            tokio::fs::metadata(&temporary).await?.len() > 1024,
            "Erzeugter Clip ist leer."
        );
        tokio::fs::rename(temporary, &video).await?;
    }
    if !thumbnail.is_file() {
        ffmpeg(
            config,
            &[
                "-i".into(),
                video.to_string_lossy().into(),
                "-frames:v".into(),
                "1".into(),
                "-vf".into(),
                "scale=640:-2".into(),
                "-q:v".into(),
                "3".into(),
                thumbnail.to_string_lossy().into(),
            ],
            Duration::from_secs(30),
        )
        .await?;
    }
    Ok((video, thumbnail))
}

#[derive(Debug)]
pub struct AudioHit {
    pub time_ms: i64,
    pub level_db: f64,
    pub baseline_db: f64,
    pub score: f64,
}

pub struct AudioDetector {
    pub threshold_db: f64,
    history: VecDeque<f64>,
    last_trigger: i64,
    sustained: usize,
}

impl AudioDetector {
    pub fn new(threshold_db: f64) -> Self {
        Self {
            threshold_db,
            history: VecDeque::new(),
            last_trigger: -100_000,
            sustained: 0,
        }
    }

    pub fn feed(&mut self, pcm: &[i16], start_ms: i64) -> (Option<AudioHit>, Option<f64>) {
        let mut hit = None;
        let mut peak: Option<f64> = None;
        for (index, window) in pcm.as_chunks::<4000>().0.iter().enumerate() {
            let rms = (window
                .iter()
                .map(|&sample| (f64::from(sample) / 32768.0).powi(2))
                .sum::<f64>()
                / window.len() as f64)
                .sqrt();
            let level = (20.0 * rms.max(1e-8).log10()).max(-96.0);
            peak = Some(peak.unwrap_or(-96.0).max(level));
            let mut sorted: Vec<f64> = self.history.iter().copied().collect();
            sorted.sort_by(f64::total_cmp);
            let baseline = if sorted.is_empty() {
                level
            } else {
                (sorted[(sorted.len() - 1) / 2] + sorted[sorted.len() / 2]) / 2.0
            };
            self.sustained = if self.history.len() >= 48
                && level > -28.0
                && level - baseline >= self.threshold_db
            {
                self.sustained + 1
            } else {
                0
            };
            let time_ms = start_ms + index as i64 * 250;
            if self.sustained >= 2 && time_ms - self.last_trigger >= 10_000 {
                hit = Some(AudioHit {
                    time_ms,
                    level_db: level,
                    baseline_db: baseline,
                    score: (45.0 + (level - baseline) * 2.0).min(99.0),
                });
                self.last_trigger = time_ms;
            }
            self.history.push_back(level);
            if self.history.len() > 240 {
                self.history.pop_front();
            }
        }
        (hit, peak)
    }
}

pub fn overlaps(a: i64, b: i64, c: i64, d: i64) -> bool {
    a < d && b > c
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tone(seconds: f64, amplitude: f64) -> Vec<i16> {
        (0..(seconds * 16000.0) as usize)
            .map(|i| {
                ((i as f64 / 16000.0 * 440.0 * std::f64::consts::TAU).sin() * amplitude * 32767.0)
                    as i16
            })
            .collect()
    }
    #[test]
    fn detects_sustained_reaction_after_warmup_and_debounces() {
        let mut detector = AudioDetector::new(9.0);
        assert!(detector.feed(&tone(16.0, 0.03), 0).0.is_none());
        let hit = detector.feed(&tone(2.0, 0.5), 16000).0.unwrap();
        assert_eq!(hit.time_ms, 16250);
        assert!(hit.level_db - hit.baseline_db > 20.0);
        assert!(detector.feed(&tone(2.0, 0.5), 18000).0.is_none());
    }
    #[test]
    fn silence_loud_background_and_single_impulses_do_not_trigger() {
        for amplitude in [0.0, 0.5] {
            assert!(
                AudioDetector::new(9.0)
                    .feed(&tone(90.0, amplitude), 0)
                    .0
                    .is_none()
            );
        }
        let mut detector = AudioDetector::new(9.0);
        detector.feed(&tone(16.0, 0.02), 0);
        let mut impulse = tone(0.25, 0.8);
        impulse.extend(tone(0.75, 0.02));
        assert!(detector.feed(&impulse, 16000).0.is_none());
    }
    #[test]
    fn rejects_path_and_symlink_escapes() {
        let directory = tempfile::tempdir().unwrap();
        assert!(safe_path(directory.path(), "../secret").is_err());
        std::os::unix::fs::symlink("/", directory.path().join("escape")).unwrap();
        assert!(safe_path(directory.path(), "escape/etc/passwd").is_err());
    }
}
