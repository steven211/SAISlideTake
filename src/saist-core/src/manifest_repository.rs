use crate::{
    audio_storage::AudioStorageService,
    error::{CoreError, CoreResult},
    types::{PresentationManifest, SlideTakeGroup},
};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tempfile::NamedTempFile;

#[derive(Debug)]
pub struct ManifestRepository {
    manifest_path: PathBuf,
    current: Mutex<PresentationManifest>,
}

impl ManifestRepository {
    pub fn open(audio: &AudioStorageService, presentation_id: &str) -> CoreResult<Self> {
        let manifest_path = audio.manifest_path();
        let current = if manifest_path.is_file() {
            let bytes = fs::read(&manifest_path)?;
            serde_json::from_slice(&bytes)?
        } else {
            let manifest = PresentationManifest::new(presentation_id);
            atomic_write(&manifest_path, &serde_json::to_vec_pretty(&manifest)?)?;
            manifest
        };
        Ok(Self {
            manifest_path,
            current: Mutex::new(current),
        })
    }

    pub fn snapshot(&self) -> CoreResult<PresentationManifest> {
        self.current
            .lock()
            .map(|manifest| manifest.clone())
            .map_err(|_| CoreError::Conflict("Manifest 狀態鎖已失效".to_string()))
    }

    pub fn group(&self, slide_id: &str) -> CoreResult<Option<SlideTakeGroup>> {
        let manifest = self
            .current
            .lock()
            .map_err(|_| CoreError::Conflict("Manifest 狀態鎖已失效".to_string()))?;
        Ok(manifest.slides.get(slide_id).cloned())
    }

    pub fn update<T>(
        &self,
        operation: impl FnOnce(&mut PresentationManifest) -> CoreResult<T>,
    ) -> CoreResult<T> {
        let mut current = self
            .current
            .lock()
            .map_err(|_| CoreError::Conflict("Manifest 狀態鎖已失效".to_string()))?;
        let mut candidate = current.clone();
        let result = operation(&mut candidate)?;
        let serialized = serde_json::to_vec_pretty(&candidate)?;
        atomic_write(&self.manifest_path, &serialized)?;
        *current = candidate;
        Ok(result)
    }
}

fn atomic_write(path: &Path, contents: &[u8]) -> CoreResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| CoreError::Invalid("Manifest 路徑缺少父目錄".to_string()))?;
    fs::create_dir_all(parent)?;
    let mut temporary = NamedTempFile::new_in(parent)?;
    temporary.write_all(contents)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .map_err(|error| CoreError::Io(error.error))?;
    Ok(())
}
