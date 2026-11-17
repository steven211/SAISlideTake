use crate::error::{CoreError, CoreResult};
use chrono::{Local, SecondsFormat};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct AudioStorageService {
    audio_root: PathBuf,
}

impl AudioStorageService {
    pub fn new(presentation_root: &Path) -> CoreResult<Self> {
        let audio_root = presentation_root.join("audio");
        fs::create_dir_all(&audio_root)?;
        Ok(Self { audio_root })
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.audio_root.join("slidetake-manifest.json")
    }

    pub fn slide_dir(&self, slide_id: &str) -> CoreResult<PathBuf> {
        validate_slide_id(slide_id)?;
        Ok(self.audio_root.join(slide_id))
    }

    pub fn audio_path(&self, slide_id: &str, filename: &str) -> CoreResult<PathBuf> {
        validate_slide_id(slide_id)?;
        validate_audio_filename(filename)?;
        Ok(self.audio_root.join(slide_id).join(filename))
    }

    pub fn next_disk_version(&self, slide_id: &str) -> CoreResult<u64> {
        let slide_dir = self.slide_dir(slide_id)?;
        let mut highest = 0_u64;
        let entries = match fs::read_dir(slide_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(1),
            Err(error) => return Err(CoreError::Io(error)),
        };
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some(rest) = name.strip_prefix("take_") else {
                continue;
            };
            let Some((version, _)) = rest.split_once('_') else {
                continue;
            };
            if let Ok(version) = version.parse::<u64>() {
                highest = highest.max(version);
            }
        }
        highest
            .checked_add(1)
            .ok_or_else(|| CoreError::Conflict("Take 版本編號已超出可用範圍".to_string()))
    }

    pub fn write_new_take(
        &self,
        slide_id: &str,
        first_version: u64,
        extension: &str,
        bytes: &[u8],
    ) -> CoreResult<(u64, String, PathBuf)> {
        if !matches!(extension, "webm" | "wav") {
            return Err(CoreError::Invalid("音訊格式只支援 WebM 或 WAV".to_string()));
        }
        let slide_dir = self.slide_dir(slide_id)?;
        fs::create_dir_all(&slide_dir)?;
        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let mut version = first_version.max(self.next_disk_version(slide_id)?).max(1);

        loop {
            let filename = format!("take_{version}_{timestamp}.{extension}");
            let destination = slide_dir.join(&filename);
            if destination.exists() {
                version = version.checked_add(1).ok_or_else(|| {
                    CoreError::Conflict("Take 版本編號已超出可用範圍".to_string())
                })?;
                continue;
            }

            let temp_path = loop {
                let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
                let path = slide_dir.join(format!(
                    ".{filename}.{}.{}.tmp",
                    std::process::id(),
                    sequence
                ));
                match OpenOptions::new().write(true).create_new(true).open(&path) {
                    Ok(mut file) => {
                        if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
                            let _ = fs::remove_file(&path);
                            return Err(CoreError::Io(error));
                        }
                        drop(file);
                        break path;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => return Err(CoreError::Io(error)),
                }
            };

            match fs::hard_link(&temp_path, &destination) {
                Ok(()) => {
                    let _ = fs::remove_file(&temp_path);
                    return Ok((version, filename, destination));
                }
                Err(error) if destination.exists() => {
                    let _ = fs::remove_file(&temp_path);
                    version = version.checked_add(1).ok_or_else(|| {
                        CoreError::Conflict("Take 版本編號已超出可用範圍".to_string())
                    })?;
                    let _ = error;
                }
                Err(error) => {
                    let _ = fs::remove_file(&temp_path);
                    return Err(CoreError::Io(error));
                }
            }
        }
    }

    pub fn remove_take(&self, slide_id: &str, filename: &str) -> CoreResult<()> {
        let path = self.audio_path(slide_id, filename)?;
        fs::remove_file(path)?;
        Ok(())
    }

    pub fn move_to_tombstone(&self, slide_id: &str, filename: &str) -> CoreResult<PathBuf> {
        let source = self.audio_path(slide_id, filename)?;
        if !source.is_file() {
            return Err(CoreError::NotFound("找不到指定的錄音檔".to_string()));
        }
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let tombstone = source.with_file_name(format!(
            ".{filename}.{}.{}.delete",
            std::process::id(),
            sequence
        ));
        fs::rename(source, &tombstone)?;
        Ok(tombstone)
    }

    pub fn restore_tombstone(
        &self,
        slide_id: &str,
        filename: &str,
        tombstone: &Path,
    ) -> CoreResult<()> {
        let destination = self.audio_path(slide_id, filename)?;
        fs::rename(tombstone, destination)?;
        Ok(())
    }

    pub fn finish_delete(&self, tombstone: &Path) -> CoreResult<()> {
        fs::remove_file(tombstone)?;
        Ok(())
    }

    pub fn now_created_at() -> String {
        Local::now().to_rfc3339_opts(SecondsFormat::Secs, false)
    }
}

pub fn validate_slide_id(slide_id: &str) -> CoreResult<()> {
    let valid = !slide_id.is_empty()
        && slide_id.len() <= 160
        && slide_id != "."
        && slide_id != ".."
        && !slide_id.chars().any(|character| {
            character.is_control() || matches!(character, '/' | '\\' | ':' | '\0')
        });
    if valid {
        Ok(())
    } else {
        Err(CoreError::Invalid("slide_id 格式不合法".to_string()))
    }
}

pub fn validate_audio_filename(filename: &str) -> CoreResult<()> {
    let path = Path::new(filename);
    let valid = !filename.is_empty()
        && filename.len() <= 240
        && path.file_name().and_then(|name| name.to_str()) == Some(filename)
        && filename.starts_with("take_")
        && (filename.ends_with(".webm") || filename.ends_with(".wav"))
        && !filename.chars().any(char::is_control);
    if valid {
        Ok(())
    } else {
        Err(CoreError::Invalid("音訊檔名格式不合法".to_string()))
    }
}
