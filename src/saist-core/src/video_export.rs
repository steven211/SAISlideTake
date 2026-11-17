use crate::{
    audio_storage::AudioStorageService,
    deck_parser::DeckSlide,
    error::{CoreError, CoreResult},
    master_export::{export_master_audio, MasterAudioExport},
    types::PresentationManifest,
};
use chrono::Local;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};
use tempfile::NamedTempFile;

static VIDEO_SEQUENCE: AtomicU64 = AtomicU64::new(0);
const MAX_SLIDE_IMAGE_BYTES: usize = 128 * 1024 * 1024;
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

#[derive(Debug, Clone, Serialize)]
pub struct VideoExportStatus {
    pub job_id: String,
    pub status: String,
    pub captured_slides: usize,
    pub total_slides: usize,
    pub filename: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VideoExportJob {
    pub job_id: String,
    pub deck_id: String,
    pub format: String,
    pub source_path: PathBuf,
    pub bridge_path: Option<PathBuf>,
    pub slides: Vec<DeckSlide>,
    pub manifest: PresentationManifest,
    pub audio_storage: AudioStorageService,
    pub images_dir: PathBuf,
    pub video_dir: PathBuf,
    status: String,
    captured_indices: HashSet<usize>,
    filename: Option<String>,
    error: Option<String>,
}

#[derive(Default)]
pub struct VideoExportManager {
    jobs: Mutex<HashMap<String, VideoExportJob>>,
}

impl VideoExportManager {
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        &self,
        deck_id: String,
        format: String,
        source_path: PathBuf,
        bridge_path: Option<PathBuf>,
        slides: Vec<DeckSlide>,
        manifest: PresentationManifest,
        audio_storage: AudioStorageService,
        presentation_root: &Path,
    ) -> CoreResult<VideoExportJob> {
        if slides.is_empty() {
            return Err(CoreError::Conflict("簡報沒有可輸出的投影片".to_string()));
        }
        let video_dir = presentation_root.join("video");
        fs::create_dir_all(&video_dir)?;
        let job_id = next_job_id();
        let images_dir = video_dir.join(".jobs").join(&job_id).join("slides");
        fs::create_dir_all(&images_dir)?;
        let status = if format == "html" {
            "capturing"
        } else {
            "processing"
        };
        let job = VideoExportJob {
            job_id: job_id.clone(),
            deck_id,
            format,
            source_path,
            bridge_path,
            slides,
            manifest,
            audio_storage,
            images_dir,
            video_dir,
            status: status.to_string(),
            captured_indices: HashSet::new(),
            filename: None,
            error: None,
        };
        self.jobs
            .lock()
            .map_err(|_| CoreError::Conflict("影片輸出工作鎖已失效".to_string()))?
            .insert(job_id, job.clone());
        Ok(job)
    }

    pub fn record_slide(
        &self,
        job_id: &str,
        slide_index: usize,
        png: &[u8],
    ) -> CoreResult<VideoExportStatus> {
        if png.len() < PNG_SIGNATURE.len()
            || png.len() > MAX_SLIDE_IMAGE_BYTES
            || !png.starts_with(PNG_SIGNATURE)
        {
            return Err(CoreError::Invalid(
                "投影片影像不是有效的 PNG 檔案".to_string(),
            ));
        }
        let mut jobs = self
            .jobs
            .lock()
            .map_err(|_| CoreError::Conflict("影片輸出工作鎖已失效".to_string()))?;
        let job = jobs
            .get_mut(job_id)
            .ok_or_else(|| CoreError::NotFound("找不到影片輸出工作".to_string()))?;
        if job.status != "capturing" {
            return Err(CoreError::Conflict(
                "影片輸出工作目前不接受投影片影像".to_string(),
            ));
        }
        if slide_index >= job.slides.len() {
            return Err(CoreError::Invalid("投影片影像頁碼超出範圍".to_string()));
        }
        if job.captured_indices.contains(&slide_index) {
            return Ok(status_of(job));
        }
        let path = job.images_dir.join(format!("slide_{slide_index:05}.png"));
        let mut temporary = NamedTempFile::new_in(&job.images_dir)?;
        temporary.write_all(png)?;
        temporary.as_file().sync_all()?;
        temporary
            .persist(&path)
            .map_err(|error| CoreError::Io(error.error))?;
        job.captured_indices.insert(slide_index);
        Ok(status_of(job))
    }

    pub fn begin_processing(&self, job_id: &str) -> CoreResult<Option<VideoExportJob>> {
        let mut jobs = self
            .jobs
            .lock()
            .map_err(|_| CoreError::Conflict("影片輸出工作鎖已失效".to_string()))?;
        let job = jobs
            .get_mut(job_id)
            .ok_or_else(|| CoreError::NotFound("找不到影片輸出工作".to_string()))?;
        if matches!(job.status.as_str(), "processing" | "completed") {
            return Ok(None);
        }
        if job.status != "capturing" {
            return Err(CoreError::Conflict("影片輸出工作狀態不正確".to_string()));
        }
        if job.captured_indices.len() != job.slides.len() {
            return Err(CoreError::Conflict(format!(
                "投影片影像尚未收齊：{}/{}",
                job.captured_indices.len(),
                job.slides.len()
            )));
        }
        job.status = "processing".to_string();
        Ok(Some(job.clone()))
    }

    pub fn get_job(&self, job_id: &str) -> CoreResult<VideoExportJob> {
        self.jobs
            .lock()
            .map_err(|_| CoreError::Conflict("影片輸出工作鎖已失效".to_string()))?
            .get(job_id)
            .cloned()
            .ok_or_else(|| CoreError::NotFound("找不到影片輸出工作".to_string()))
    }

    pub fn mark_slides_ready(&self, job_id: &str) -> CoreResult<()> {
        let mut jobs = self
            .jobs
            .lock()
            .map_err(|_| CoreError::Conflict("影片輸出工作鎖已失效".to_string()))?;
        let job = jobs
            .get_mut(job_id)
            .ok_or_else(|| CoreError::NotFound("找不到影片輸出工作".to_string()))?;
        if job.format != "pptx" || job.status != "processing" {
            return Err(CoreError::Conflict("影片輸出工作狀態不正確".to_string()));
        }
        job.captured_indices.extend(0..job.slides.len());
        Ok(())
    }

    pub fn status(&self, job_id: &str) -> CoreResult<VideoExportStatus> {
        self.jobs
            .lock()
            .map_err(|_| CoreError::Conflict("影片輸出工作鎖已失效".to_string()))?
            .get(job_id)
            .map(status_of)
            .ok_or_else(|| CoreError::NotFound("找不到影片輸出工作".to_string()))
    }

    pub fn path_for_filename(&self, filename: &str) -> CoreResult<PathBuf> {
        if !filename.starts_with("tutorial_")
            || !filename.ends_with(".mp4")
            || Path::new(filename)
                .file_name()
                .and_then(|name| name.to_str())
                != Some(filename)
        {
            return Err(CoreError::Invalid("影片檔名不合法".to_string()));
        }
        self.jobs
            .lock()
            .map_err(|_| CoreError::Conflict("影片輸出工作鎖已失效".to_string()))?
            .values()
            .find(|job| job.filename.as_deref() == Some(filename))
            .map(|job| job.video_dir.join(filename))
            .ok_or_else(|| CoreError::NotFound("找不到匯出的影片".to_string()))
    }

    pub fn mark_completed(&self, job_id: &str, filename: String) -> CoreResult<()> {
        let mut jobs = self
            .jobs
            .lock()
            .map_err(|_| CoreError::Conflict("影片輸出工作鎖已失效".to_string()))?;
        let job = jobs
            .get_mut(job_id)
            .ok_or_else(|| CoreError::NotFound("找不到影片輸出工作".to_string()))?;
        job.status = "completed".to_string();
        job.filename = Some(filename);
        job.error = None;
        Ok(())
    }

    pub fn mark_failed(&self, job_id: &str, error: String) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(job) = jobs.get_mut(job_id) {
                if matches!(job.status.as_str(), "capturing" | "processing") {
                    job.status = "failed".to_string();
                    job.error = Some(error);
                }
            }
        }
    }
}

pub fn render_video(
    job: &VideoExportJob,
    slides_ready: impl FnOnce(usize) -> CoreResult<()>,
) -> CoreResult<(String, MasterAudioExport)> {
    if job.format == "pptx" {
        render_pptx_slide_images(&job.source_path, &job.images_dir)?;
    }
    let images = slide_image_paths(job)?;
    slides_ready(images.len())?;
    let slide_ids = job
        .slides
        .iter()
        .map(|slide| slide.slide_id.clone())
        .collect::<Vec<_>>();
    let master = export_master_audio(&job.audio_storage, &job.manifest, &slide_ids)?;
    let audio_path = job
        .audio_storage
        .manifest_path()
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(&master.filename);
    let working = tempfile::Builder::new()
        .prefix(".video-export-")
        .tempdir_in(&job.video_dir)?;
    let concat_path = working.path().join("slides.txt");
    let mut concat = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&concat_path)?;
    for (index, (slide, image)) in job.slides.iter().zip(&images).enumerate() {
        let duration = slide_duration(slide.slide_id.as_str(), &job.manifest);
        let image_path = image
            .to_str()
            .ok_or_else(|| CoreError::Invalid("投影片影像路徑無效".to_string()))?;
        writeln!(
            concat,
            "file '{}'\nduration {:.6}",
            escape_concat_path(image_path),
            duration
        )?;
        if index + 1 == job.slides.len() {
            writeln!(
                concat,
                "file '{}'\nduration 3600",
                escape_concat_path(image_path)
            )?;
        }
    }
    concat.sync_all()?;

    let staged = working.path().join("tutorial.mp4");
    run_ffmpeg([
        "-f".to_string(),
        "concat".to_string(),
        "-safe".to_string(),
        "0".to_string(),
        "-i".to_string(),
        concat_path.to_string_lossy().into_owned(),
        "-i".to_string(),
        audio_path.to_string_lossy().into_owned(),
        "-map".to_string(),
        "0:v:0".to_string(),
        "-map".to_string(),
        "1:a:0".to_string(),
        "-vf".to_string(),
        "scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2:color=black,setsar=1".to_string(),
        "-fps_mode".to_string(),
        "vfr".to_string(),
        "-c:v".to_string(),
        "libx264".to_string(),
        "-preset".to_string(),
        "medium".to_string(),
        "-crf".to_string(),
        "20".to_string(),
        "-pix_fmt".to_string(),
        "yuv420p".to_string(),
        "-c:a".to_string(),
        "aac".to_string(),
        "-b:a".to_string(),
        "192k".to_string(),
        "-shortest".to_string(),
        "-movflags".to_string(),
        "+faststart".to_string(),
        "-y".to_string(),
        staged.to_string_lossy().into_owned(),
    ])?;

    let filename = loop {
        let candidate = next_video_filename();
        match fs::hard_link(&staged, job.video_dir.join(&candidate)) {
            Ok(()) => break candidate,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(CoreError::Io(error)),
        }
    };
    Ok((filename, master))
}

fn slide_image_paths(job: &VideoExportJob) -> CoreResult<Vec<PathBuf>> {
    let mut paths = Vec::with_capacity(job.slides.len());
    for index in 0..job.slides.len() {
        let path = job.images_dir.join(format!("slide_{index:05}.png"));
        let metadata = fs::metadata(&path).map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => {
                CoreError::Conflict(format!("缺少第 {} 頁的投影片影像", index + 1))
            }
            _ => CoreError::Io(error),
        })?;
        if !metadata.is_file() || metadata.len() < PNG_SIGNATURE.len() as u64 {
            return Err(CoreError::Invalid(format!(
                "第 {} 頁投影片影像無效",
                index + 1
            )));
        }
        paths.push(path);
    }
    Ok(paths)
}

fn slide_duration(slide_id: &str, manifest: &PresentationManifest) -> f64 {
    let golden_duration = manifest.slides.get(slide_id).and_then(|group| {
        let golden_id = group.golden_take_id.as_deref()?;
        group
            .takes
            .iter()
            .find(|take| take.take_id == golden_id)
            .map(|take| take.duration_sec)
    });
    golden_duration
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .unwrap_or_else(|| (manifest.transition_delay_ms.max(100) as f64) / 1000.0)
}

fn render_pptx_slide_images(source: &Path, output: &Path) -> CoreResult<()> {
    if let Some(renderer) = std::env::var_os("SAIST_PPTX_RENDERER") {
        let result = Command::new(renderer).arg(source).arg(output).output()?;
        if !result.status.success() {
            return Err(CoreError::Conflict(render_error(
                "PPTX 投影片轉圖失敗",
                &result.stderr,
            )));
        }
        return normalize_png_sequence(output);
    }
    #[cfg(target_os = "macos")]
    {
        const SCRIPT: &str = r#"on run argv
set sourcePath to POSIX file (item 1 of argv)
set outputPath to POSIX file (item 2 of argv)
tell application id "com.apple.iWork.Keynote"
    set deckDocument to open sourcePath
    export deckDocument to outputPath as slide images with properties {image format:PNG}
    close deckDocument saving no
end tell
end run"#;
        let result = Command::new("/usr/bin/osascript")
            .args([
                "-e",
                SCRIPT,
                source.to_string_lossy().as_ref(),
                output.to_string_lossy().as_ref(),
            ])
            .output()?;
        if !result.status.success() {
            return Err(CoreError::Conflict(render_error(
                "PPTX 轉圖需要可用的 Keynote，或設定 SAIST_PPTX_RENDERER",
                &result.stderr,
            )));
        }
        normalize_png_sequence(output)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (source, output);
        Err(CoreError::Conflict(
            "目前平台尚未設定 PPTX 投影片轉圖器".to_string(),
        ))
    }
}

fn normalize_png_sequence(directory: &Path) -> CoreResult<()> {
    fn collect_pngs(directory: &Path, output: &mut Vec<PathBuf>) -> CoreResult<()> {
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            if path.is_dir() {
                collect_pngs(&path, output)?;
            } else if path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
            {
                let stem = path
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default();
                let already_numbered = stem
                    .strip_prefix("slide_")
                    .is_some_and(|number| number.len() == 5 && number.parse::<usize>().is_ok());
                if !already_numbered {
                    output.push(path);
                }
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    collect_pngs(directory, &mut files)?;
    files.sort_by_key(|path| {
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default();
        let digits = name
            .chars()
            .rev()
            .take_while(|character| character.is_ascii_digit())
            .collect::<String>();
        (
            digits
                .chars()
                .rev()
                .collect::<String>()
                .parse::<u64>()
                .unwrap_or(u64::MAX),
            name.to_lowercase(),
        )
    });
    for (index, path) in files.into_iter().enumerate() {
        let target = directory.join(format!("slide_{index:05}.png"));
        if path != target {
            fs::copy(path, target)?;
        }
    }
    Ok(())
}

fn run_ffmpeg<const N: usize>(args: [String; N]) -> CoreResult<()> {
    let executable = if let Some(path) = std::env::var_os("SAIST_FFMPEG") {
        PathBuf::from(path)
    } else if cfg!(target_os = "macos") && Path::new("/opt/homebrew/bin/ffmpeg").is_file() {
        PathBuf::from("/opt/homebrew/bin/ffmpeg")
    } else if cfg!(target_os = "macos") && Path::new("/usr/local/bin/ffmpeg").is_file() {
        PathBuf::from("/usr/local/bin/ffmpeg")
    } else {
        PathBuf::from(if cfg!(windows) {
            "ffmpeg.exe"
        } else {
            "ffmpeg"
        })
    };
    let output = Command::new(executable)
        .args(["-nostdin", "-hide_banner", "-loglevel", "error"])
        .args(args)
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                CoreError::Conflict("找不到 FFmpeg；請安裝 ffmpeg 或設定 SAIST_FFMPEG".to_string())
            } else {
                CoreError::Io(error)
            }
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(CoreError::Conflict(render_error(
            "FFmpeg 影片合成失敗",
            &output.stderr,
        )))
    }
}

fn render_error(prefix: &str, bytes: &[u8]) -> String {
    let detail = String::from_utf8_lossy(bytes)
        .trim()
        .chars()
        .take(500)
        .collect::<String>();
    if detail.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}：{detail}")
    }
}

fn escape_concat_path(path: &str) -> String {
    path.replace('\\', "\\\\").replace('\'', "'\\''")
}

fn status_of(job: &VideoExportJob) -> VideoExportStatus {
    VideoExportStatus {
        job_id: job.job_id.clone(),
        status: job.status.clone(),
        captured_slides: job.captured_indices.len(),
        total_slides: job.slides.len(),
        filename: job.filename.clone(),
        error: job.error.clone(),
    }
}

fn next_job_id() -> String {
    let timestamp = Local::now().format("%Y%m%d%H%M%S%3f");
    let sequence = VIDEO_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("video_{timestamp}_{sequence}")
}

fn next_video_filename() -> String {
    let timestamp = Local::now().format("%Y%m%d_%H%M%S_%3f");
    let sequence = VIDEO_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("tutorial_{timestamp}_{sequence}.mp4")
}
