use crate::{
    audio_storage::AudioStorageService,
    error::{CoreError, CoreResult},
    types::PresentationManifest,
};
use chrono::Local;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
use tempfile::Builder;

static EXPORT_SEQUENCE: AtomicU64 = AtomicU64::new(0);
const MASTER_SAMPLE_RATE: u32 = 48_000;

#[derive(Debug, Clone, serde::Serialize)]
pub struct MasterAudioExport {
    pub filename: String,
    pub relative_path: String,
    pub duration_sec: f64,
    pub slide_count: usize,
}

pub fn export_master_audio(
    storage: &AudioStorageService,
    manifest: &PresentationManifest,
    ordered_slide_ids: &[String],
) -> CoreResult<MasterAudioExport> {
    if ordered_slide_ids.is_empty() {
        return Err(CoreError::Conflict(
            "請先拖入 HTML 或 PPTX 簡報".to_string(),
        ));
    }
    let ffmpeg = ffmpeg_executable();
    let manifest_path = storage.manifest_path();
    let audio_root = manifest_path
        .parent()
        .ok_or_else(|| CoreError::Invalid("音訊目錄不存在".to_string()))?;
    fs::create_dir_all(audio_root)?;
    let working = Builder::new()
        .prefix(".master-export-")
        .tempdir_in(audio_root)?;
    let mut parts = Vec::new();
    let mut duration_sec = 0.0_f64;

    for (index, slide_id) in ordered_slide_ids.iter().enumerate() {
        let group = manifest.slides.get(slide_id);
        let golden = group.and_then(|group| {
            let id = group.golden_take_id.as_deref()?;
            group.takes.iter().find(|take| take.take_id == id)
        });
        let part = working.path().join(format!("part_{index:05}.wav"));
        if let Some(take) = golden {
            duration_sec += take.duration_sec;
            let input = storage.audio_path(slide_id, &take.filename)?;
            let metadata = fs::metadata(&input).map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => {
                    CoreError::NotFound(format!("找不到黃金錄音：{}", take.filename))
                }
                _ => CoreError::Io(error),
            })?;
            if !metadata.is_file() || metadata.len() == 0 {
                return Err(CoreError::Invalid(format!(
                    "黃金錄音無法使用：{}",
                    take.filename
                )));
            }
            run_ffmpeg(
                &ffmpeg,
                [
                    "-i".to_string(),
                    input.to_string_lossy().into_owned(),
                    "-map".to_string(),
                    "0:a:0".to_string(),
                    "-vn".to_string(),
                    "-ar".to_string(),
                    MASTER_SAMPLE_RATE.to_string(),
                    "-ac".to_string(),
                    "2".to_string(),
                    "-c:a".to_string(),
                    "pcm_s16le".to_string(),
                    "-f".to_string(),
                    "wav".to_string(),
                    "-y".to_string(),
                    part.to_string_lossy().into_owned(),
                ],
            )?;
        } else if manifest.transition_delay_ms > 0 {
            duration_sec += manifest.transition_delay_ms as f64 / 1000.0;
            let duration = format!("{:.3}", manifest.transition_delay_ms as f64 / 1000.0);
            run_ffmpeg(
                &ffmpeg,
                [
                    "-f".to_string(),
                    "lavfi".to_string(),
                    "-i".to_string(),
                    format!("anullsrc=r={MASTER_SAMPLE_RATE}:cl=stereo"),
                    "-t".to_string(),
                    duration,
                    "-ar".to_string(),
                    MASTER_SAMPLE_RATE.to_string(),
                    "-ac".to_string(),
                    "2".to_string(),
                    "-c:a".to_string(),
                    "pcm_s16le".to_string(),
                    "-f".to_string(),
                    "wav".to_string(),
                    "-y".to_string(),
                    part.to_string_lossy().into_owned(),
                ],
            )?;
        }
        if part.is_file() {
            parts.push(part);
        }
    }
    if parts.is_empty() {
        return Err(CoreError::Conflict(
            "目前沒有可匯出的黃金錄音或轉場靜音".to_string(),
        ));
    }

    let list_path = working.path().join("concat.txt");
    let mut list = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&list_path)?;
    for part in &parts {
        let name = part
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| CoreError::Invalid("母帶暫存檔名無效".to_string()))?;
        writeln!(list, "file '{name}'")?;
    }
    list.sync_all()?;

    let staged_output = working.path().join("master.wav");
    run_ffmpeg(
        &ffmpeg,
        [
            "-f".to_string(),
            "concat".to_string(),
            "-i".to_string(),
            list_path.to_string_lossy().into_owned(),
            "-map".to_string(),
            "0:a:0".to_string(),
            "-c:a".to_string(),
            "pcm_s16le".to_string(),
            "-ar".to_string(),
            MASTER_SAMPLE_RATE.to_string(),
            "-ac".to_string(),
            "2".to_string(),
            "-f".to_string(),
            "wav".to_string(),
            "-y".to_string(),
            staged_output.to_string_lossy().into_owned(),
        ],
    )?;
    let filename = loop {
        let candidate = next_master_filename();
        let destination = audio_root.join(&candidate);
        match fs::hard_link(&staged_output, &destination) {
            Ok(()) => break candidate,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(CoreError::Io(error)),
        }
    };
    Ok(MasterAudioExport {
        relative_path: format!("audio/{filename}"),
        filename,
        duration_sec,
        slide_count: ordered_slide_ids.len(),
    })
}

fn ffmpeg_executable() -> PathBuf {
    if let Some(path) = std::env::var_os("SAIST_FFMPEG") {
        return PathBuf::from(path);
    }
    #[cfg(target_os = "macos")]
    for path in ["/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg"] {
        if Path::new(path).is_file() {
            return PathBuf::from(path);
        }
    }
    PathBuf::from(if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    })
}

fn run_ffmpeg<const N: usize>(executable: &Path, args: [String; N]) -> CoreResult<()> {
    let output = Command::new(executable)
        .arg("-nostdin")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .args(args)
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                CoreError::Conflict(
                    "找不到 FFmpeg；請安裝 ffmpeg 或設定 SAIST_FFMPEG 指向執行檔".to_string(),
                )
            } else {
                CoreError::Io(error)
            }
        })?;
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr)
        .trim()
        .chars()
        .take(500)
        .collect::<String>();
    Err(CoreError::Conflict(if detail.is_empty() {
        "FFmpeg 音訊轉碼失敗".to_string()
    } else {
        format!("FFmpeg 音訊轉碼失敗：{detail}")
    }))
}

fn next_master_filename() -> String {
    let timestamp = Local::now().format("%Y%m%d_%H%M%S_%3f");
    loop {
        let sequence = EXPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let filename = format!("master_export_{timestamp}_{sequence}.wav");
        return filename;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        audio_storage::AudioStorageService,
        types::{PresentationManifest, SlideTakeGroup, TakeCard},
    };
    use std::{io::Read, process::Command};
    use tempfile::TempDir;

    #[test]
    fn transcodes_golden_webm_and_inserts_silence_for_missing_slides() {
        if Command::new(ffmpeg_executable())
            .arg("-version")
            .output()
            .is_err()
        {
            return;
        }
        let root = TempDir::new().unwrap();
        let storage = AudioStorageService::new(root.path()).unwrap();
        let slide_dir = storage.slide_dir("intro").unwrap();
        fs::create_dir_all(&slide_dir).unwrap();
        let source = slide_dir.join("take_1_20261111_120000.webm");
        let generated = Command::new(ffmpeg_executable())
            .args([
                "-nostdin",
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=0.25",
                "-c:a",
                "libopus",
                "-f",
                "webm",
                "-y",
            ])
            .arg(&source)
            .output()
            .unwrap();
        assert!(
            generated.status.success(),
            "{}",
            String::from_utf8_lossy(&generated.stderr)
        );

        let mut manifest = PresentationManifest::new("deck");
        manifest.transition_delay_ms = 400;
        manifest.slides.insert(
            "intro".to_string(),
            SlideTakeGroup {
                slide_id: "intro".to_string(),
                slide_title: "Intro".to_string(),
                golden_take_id: Some("take_1".to_string()),
                next_take_number: 2,
                takes: vec![TakeCard {
                    take_id: "take_1".to_string(),
                    slide_id: "intro".to_string(),
                    filename: "take_1_20261111_120000.webm".to_string(),
                    relative_path: "audio/intro/take_1_20261111_120000.webm".to_string(),
                    duration_sec: 0.25,
                    created_at: "2026-11-11T12:00:00+08:00".to_string(),
                    note: String::new(),
                    is_golden: true,
                }],
            },
        );
        let export = export_master_audio(
            &storage,
            &manifest,
            &["intro".to_string(), "missing".to_string()],
        )
        .unwrap();
        let output = storage
            .manifest_path()
            .parent()
            .unwrap()
            .join(&export.filename);
        let mut file = fs::File::open(output).unwrap();
        let mut header = [0_u8; 12];
        file.read_exact(&mut header).unwrap();
        assert_eq!(&header[..4], b"RIFF");
        assert_eq!(&header[8..], b"WAVE");
        assert_eq!(export.slide_count, 2);
        assert!(export.duration_sec >= 0.4);
    }
}
