use crate::{
    audio_storage::{validate_slide_id, AudioStorageService},
    deck_parser::{
        parse_deck, parse_html_with_profile, profile_bridge_slide_count, test_html_parser_profile,
        DeckSlide, ParsedDeck,
    },
    error::{CoreError, CoreResult},
    manifest_repository::ManifestRepository,
    master_export::{export_master_audio, MasterAudioExport},
    recording_service::{AudioRecordingService, RecordingStatus},
    types::{
        ActiveSlideInfo, DeckParserProfile, ParserSlidePreview, ParserTestResult,
        PresentationManifest, SlideTakeGroup, TakeCard,
    },
    video_export::{render_video, VideoExportManager, VideoExportStatus},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex, RwLock,
    },
};
use tempfile::NamedTempFile;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeckInfo {
    pub deck_id: String,
    pub title: String,
    pub source_path: String,
    pub format: String,
    pub bridge_path: Option<String>,
    pub slides: Vec<DeckSlide>,
}

struct DeckContext {
    deck_id: String,
    presentation_root: PathBuf,
    audio_storage: AudioStorageService,
    manifest_repository: ManifestRepository,
    parsed_deck: Option<ParsedDeck>,
}

impl DeckContext {
    fn info(&self) -> Option<DeckInfo> {
        let deck = self.parsed_deck.as_ref()?;
        Some(DeckInfo {
            deck_id: self.deck_id.clone(),
            title: deck.title.clone(),
            source_path: deck.source_path.clone(),
            format: deck.format.clone(),
            bridge_path: deck.bridge_path.clone(),
            slides: deck.slides.clone(),
        })
    }
}

#[derive(Clone)]
pub struct SlideSyncHub {
    sender: broadcast::Sender<String>,
    presentation_ui_connections: Arc<AtomicUsize>,
}

impl SlideSyncHub {
    fn new() -> Self {
        let (sender, _) = broadcast::channel(256);
        Self {
            sender,
            presentation_ui_connections: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.sender.subscribe()
    }

    pub fn publish(&self, event_type: &str, payload: Value) {
        let event = json!({"type": event_type, "payload": payload}).to_string();
        let _ = self.sender.send(event);
    }

    pub fn attach_presentation_ui(&self) -> usize {
        let count = self
            .presentation_ui_connections
            .fetch_add(1, Ordering::AcqRel)
            + 1;
        self.publish("embedded_ui_attached", json!({"count": count}));
        count
    }

    pub fn detach_presentation_ui(&self) -> usize {
        let count = self
            .presentation_ui_connections
            .try_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                Some(count.saturating_sub(1))
            })
            .unwrap_or_default()
            .saturating_sub(1);
        self.publish("embedded_ui_detached", json!({"count": count}));
        count
    }

    pub fn presentation_ui_connection_count(&self) -> usize {
        self.presentation_ui_connections.load(Ordering::Acquire)
    }
}

pub struct AppContainer {
    active_deck: RwLock<Arc<DeckContext>>,
    workspace_root: PathBuf,
    pub slide_sync_hub: SlideSyncHub,
    active_slide: RwLock<Option<ActiveSlideInfo>>,
    mutation_lock: Mutex<()>,
    watcher_generation: AtomicU64,
    recording_service: AudioRecordingService,
    video_exports: VideoExportManager,
}

impl Drop for AppContainer {
    fn drop(&mut self) {
        if self.recording_service.is_recording() {
            if let Err(error) = self.stop_recording() {
                eprintln!("[SAISlideTake] Could not finalize recording on shutdown: {error}");
            }
        }
    }
}

impl AppContainer {
    pub fn open(presentation_root: &Path) -> CoreResult<Arc<Self>> {
        std::fs::create_dir_all(presentation_root)?;
        let presentation_id = presentation_root
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("presentation");
        let audio_storage = AudioStorageService::new(presentation_root)?;
        let manifest_repository = ManifestRepository::open(&audio_storage, presentation_id)?;
        let context = Arc::new(DeckContext {
            deck_id: presentation_id.to_string(),
            presentation_root: presentation_root.to_path_buf(),
            audio_storage,
            manifest_repository,
            parsed_deck: None,
        });
        Ok(Arc::new(Self {
            active_deck: RwLock::new(context),
            workspace_root: presentation_root.join("slidetake_decks"),
            slide_sync_hub: SlideSyncHub::new(),
            active_slide: RwLock::new(None),
            mutation_lock: Mutex::new(()),
            watcher_generation: AtomicU64::new(0),
            recording_service: AudioRecordingService::default(),
            video_exports: VideoExportManager::default(),
        }))
    }

    fn deck_context(&self) -> CoreResult<Arc<DeckContext>> {
        self.active_deck
            .read()
            .map(|deck| deck.clone())
            .map_err(|_| CoreError::Conflict("簡報工作區鎖已失效".to_string()))
    }

    fn resolve_or_relink_deck_id(&self, source_path: &Path, parsed: &ParsedDeck) -> String {
        let stem = source_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("deck");
        let default_id = stable_deck_id(stem, &source_path.to_string_lossy());
        let decks_dir = self.workspace_root.join("decks");
        if !decks_dir.exists() {
            return default_id;
        }

        // 1. 若當前路徑生成的 default_id 目錄已存在且包含有效設定，直接沿用
        let default_path = decks_dir.join(&default_id);
        if default_path.join("deck.json").exists() {
            return default_id;
        }

        // 2. 否則在 decks 目錄中搜尋是否存在特徵相符的既有專案（異機遷移/路徑變更自動重連）
        let entries = match fs::read_dir(&decks_dir) {
            Ok(e) => e,
            Err(_) => return default_id,
        };

        let incoming_filename = source_path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("");
        let incoming_slide_ids: std::collections::HashSet<&str> =
            parsed.slides.iter().map(|s| s.slide_id.as_str()).collect();

        let mut best_match: Option<(String, usize)> = None;

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let deck_json = path.join("deck.json");
            if !deck_json.exists() {
                continue;
            }

            if let Ok(content) = fs::read_to_string(&deck_json) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                    let old_source = val["deck"]["source_path"].as_str().unwrap_or("");
                    let old_filename = Path::new(old_source)
                        .file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or("");

                    // 計算投影片 ID 匹配數量
                    let mut matched_slides = 0;
                    if let Some(slides) = val["deck"]["slides"].as_array() {
                        for s in slides {
                            if let Some(sid) = s["slide_id"].as_str() {
                                if incoming_slide_ids.contains(sid) {
                                    matched_slides += 1;
                                }
                            }
                        }
                    }

                    let is_old_path_stale = !Path::new(old_source).exists();
                    let same_filename =
                        !incoming_filename.is_empty() && incoming_filename == old_filename;

                    // 判定重連條件：
                    // 條件 1: 檔名相同，且舊路徑已失效（異機搬移常見）或具有投影片交集
                    // 條件 2: 投影片交集涵蓋超過半數（即便簡報被重新命名）
                    let is_candidate = (same_filename && (is_old_path_stale || matched_slides > 0))
                        || (matched_slides > 0 && matched_slides * 2 >= incoming_slide_ids.len());

                    if is_candidate {
                        let score = matched_slides + if same_filename { 100 } else { 0 };
                        if best_match.as_ref().map_or(true, |(_, s)| score > *s) {
                            if let Some(existing_deck_id) = val["deck_id"]
                                .as_str()
                                .or_else(|| path.file_name().and_then(|f| f.to_str()))
                            {
                                best_match = Some((existing_deck_id.to_string(), score));
                            }
                        }
                    }
                }
            }
        }

        if let Some((matched_id, score)) = best_match {
            println!(
                "[SAISlideTake] 自動重連至既有錄音專案: {} (匹配特徵權重: {})",
                matched_id, score
            );
            return matched_id;
        }

        default_id
    }

    pub fn deck_info(&self) -> CoreResult<Option<DeckInfo>> {
        Ok(self.deck_context()?.info())
    }

    pub fn audio_path(&self, slide_id: &str, filename: &str) -> CoreResult<PathBuf> {
        self.deck_context()?
            .audio_storage
            .audio_path(slide_id, filename)
    }

    pub fn master_audio_path(&self, filename: &str) -> CoreResult<PathBuf> {
        let valid = filename.starts_with("master_export_")
            && filename.ends_with(".wav")
            && Path::new(filename)
                .file_name()
                .and_then(|name| name.to_str())
                == Some(filename);
        if !valid {
            return Err(CoreError::Invalid("母帶檔名不合法".to_string()));
        }
        let context = self.deck_context()?;
        Ok(context
            .audio_storage
            .manifest_path()
            .parent()
            .unwrap_or(&context.presentation_root)
            .join(filename))
    }

    pub fn ingest_deck(
        self: &Arc<Self>,
        source_path: &Path,
        parser_profile: Option<DeckParserProfile>,
    ) -> CoreResult<DeckInfo> {
        if self.recording_service.is_recording() {
            return Err(CoreError::Conflict("請先停止錄音，再切換簡報".to_string()));
        }
        let source_path = source_path.canonicalize()?;
        let effective_profile = if source_path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| {
                value.eq_ignore_ascii_case("html") || value.eq_ignore_ascii_case("htm")
            }) {
            parser_profile
        } else {
            None
        };
        let mut parsed = match effective_profile.as_ref() {
            Some(profile) => parse_html_with_profile(&source_path, profile)?,
            None => parse_deck(&source_path)?,
        };
        parsed.source_path = source_path.to_string_lossy().into_owned();
        if parsed.format == "html" {
            parsed.bridge_path = Some(
                inject_bridge_sidecar(&source_path, effective_profile.as_ref())?
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        let deck_id = self.resolve_or_relink_deck_id(&source_path, &parsed);
        let presentation_root = self.workspace_root.join("decks").join(&deck_id);
        fs::create_dir_all(&presentation_root)?;
        let audio_storage = AudioStorageService::new(&presentation_root)?;
        let manifest_repository = ManifestRepository::open(&audio_storage, &deck_id)?;
        manifest_repository.update(|manifest| {
            manifest.presentation_id = deck_id.clone();
            for slide in &parsed.slides {
                let group = manifest
                    .slides
                    .entry(slide.slide_id.clone())
                    .or_insert_with(|| SlideTakeGroup {
                        slide_id: slide.slide_id.clone(),
                        slide_title: slide.title.clone(),
                        golden_take_id: None,
                        next_take_number: 1,
                        takes: Vec::new(),
                    });
                group.slide_title = slide.title.clone();
            }
            Ok(())
        })?;
        persist_deck_info(&presentation_root, &deck_id, &parsed)?;
        let context = Arc::new(DeckContext {
            deck_id: deck_id.clone(),
            presentation_root,
            audio_storage,
            manifest_repository,
            parsed_deck: Some(parsed.clone()),
        });
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| CoreError::Conflict("錄音寫入狀態鎖已失效".to_string()))?;
        if self.recording_service.is_recording() {
            return Err(CoreError::Conflict("請先停止錄音，再切換簡報".to_string()));
        }
        *self
            .active_deck
            .write()
            .map_err(|_| CoreError::Conflict("簡報工作區鎖已失效".to_string()))? = context;
        *self
            .active_slide
            .write()
            .map_err(|_| CoreError::Conflict("目前頁面狀態鎖已失效".to_string()))? = None;
        let info = self
            .deck_info()?
            .ok_or_else(|| CoreError::Conflict("簡報載入失敗".to_string()))?;
        let generation = self.watcher_generation.fetch_add(1, Ordering::Relaxed) + 1;
        if info.format == "pptx" {
            crate::slideshow_watcher::start(Arc::downgrade(self), info.deck_id.clone(), generation);
        }
        self.slide_sync_hub.publish("deck_loaded", json!(info));
        if let Some(profile) = effective_profile {
            self.slide_sync_hub.publish(
                "deck_parser_profile_updated",
                json!({"deck_id": info.deck_id.clone(), "bridge_path": info.bridge_path.clone(), "profile": profile}),
            );
        }
        Ok(info)
    }

    pub fn test_parser_profile(
        &self,
        profile: Option<&DeckParserProfile>,
    ) -> CoreResult<ParserTestResult> {
        let context = self.deck_context()?;
        let deck = context
            .parsed_deck
            .as_ref()
            .ok_or_else(|| CoreError::NotFound("請先載入簡報，再測試解析規則".to_string()))?;
        if deck.format == "html" && profile.is_some() {
            let profile = profile.expect("checked above");
            return test_html_parser_profile(Path::new(&deck.source_path), profile);
        }
        let parsed = parse_deck(Path::new(&deck.source_path))?;
        let slides = parsed
            .slides
            .into_iter()
            .map(|slide| ParserSlidePreview {
                slide_index: slide.slide_index,
                title: slide.title,
                notes: slide.notes,
                active: false,
            })
            .collect::<Vec<_>>();
        Ok(ParserTestResult {
            total_slides: slides.len(),
            slides,
            error: None,
        })
    }

    pub fn apply_parser_profile(
        self: &Arc<Self>,
        profile: DeckParserProfile,
    ) -> CoreResult<DeckInfo> {
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| CoreError::Conflict("錄音寫入狀態鎖已失效".to_string()))?;
        if self.recording_service.is_recording() {
            return Err(CoreError::Conflict(
                "請先停止錄音，再套用解析規則".to_string(),
            ));
        }
        let context = self.deck_context()?;
        let current = context
            .parsed_deck
            .as_ref()
            .ok_or_else(|| CoreError::NotFound("請先載入 HTML 簡報，再套用解析規則".to_string()))?;
        if current.format != "html" {
            return Err(CoreError::Invalid(
                "PPTX 使用內建 OpenXML 解析器，不需套用 HTML 規則".to_string(),
            ));
        }
        let source_path = PathBuf::from(&current.source_path);
        let mut parsed = parse_html_with_profile(&source_path, &profile)?;
        let bridge_slide_count = profile_bridge_slide_count(&source_path, &profile)?;
        if bridge_slide_count != parsed.slides.len() {
            return Err(CoreError::Invalid(
                "此規則可預覽，但無法對應到簡報 DOM 分頁；請調整 slide_selector 或標題備援規則"
                    .to_string(),
            ));
        }
        parsed.bridge_path = current.bridge_path.clone();
        context.manifest_repository.update(|manifest| {
            for slide in &parsed.slides {
                let group = manifest
                    .slides
                    .entry(slide.slide_id.clone())
                    .or_insert_with(|| SlideTakeGroup {
                        slide_id: slide.slide_id.clone(),
                        slide_title: slide.title.clone(),
                        golden_take_id: None,
                        next_take_number: 1,
                        takes: Vec::new(),
                    });
                group.slide_title = slide.title.clone();
            }
            Ok(())
        })?;
        persist_deck_info(&context.presentation_root, &context.deck_id, &parsed)?;
        let repository = ManifestRepository::open(&context.audio_storage, &context.deck_id)?;
        let next = Arc::new(DeckContext {
            deck_id: context.deck_id.clone(),
            presentation_root: context.presentation_root.clone(),
            audio_storage: context.audio_storage.clone(),
            manifest_repository: repository,
            parsed_deck: Some(parsed),
        });
        *self
            .active_deck
            .write()
            .map_err(|_| CoreError::Conflict("簡報工作區鎖已失效".to_string()))? = next;
        *self
            .active_slide
            .write()
            .map_err(|_| CoreError::Conflict("目前頁面狀態鎖已失效".to_string()))? = None;
        let info = self
            .deck_info()?
            .ok_or_else(|| CoreError::Conflict("套用解析規則後無法讀取簡報".to_string()))?;
        self.slide_sync_hub.publish("deck_loaded", json!(info));
        self.slide_sync_hub.publish(
            "deck_parser_profile_updated",
            json!({"deck_id": info.deck_id.clone(), "bridge_path": info.bridge_path.clone(), "profile": profile}),
        );
        Ok(info)
    }

    pub(crate) fn watcher_is_current(&self, deck_id: &str, generation: u64) -> bool {
        self.watcher_generation.load(Ordering::Relaxed) == generation
            && self
                .deck_context()
                .is_ok_and(|context| context.deck_id == deck_id)
    }

    pub fn export_master_audio(&self) -> CoreResult<MasterAudioExport> {
        let context = self.deck_context()?;
        let deck = context
            .parsed_deck
            .as_ref()
            .ok_or_else(|| CoreError::Conflict("請先載入一份 HTML 或 PPTX 簡報".to_string()))?;
        let manifest = context.manifest_repository.snapshot()?;
        let slide_ids = deck
            .slides
            .iter()
            .map(|slide| slide.slide_id.clone())
            .collect::<Vec<_>>();
        export_master_audio(&context.audio_storage, &manifest, &slide_ids)
    }

    pub fn start_video_export(self: &Arc<Self>) -> CoreResult<VideoExportStatus> {
        let context = self.deck_context()?;
        let deck = context
            .parsed_deck
            .as_ref()
            .ok_or_else(|| CoreError::Conflict("請先載入一份 HTML 或 PPTX 簡報".to_string()))?;
        if deck.format == "html" && deck.bridge_path.is_none() {
            return Err(CoreError::Conflict(
                "HTML 簡報尚未建立 SAISlideTake Bridge，請重新載入簡報".to_string(),
            ));
        }
        if deck.format == "html" && self.slide_sync_hub.presentation_ui_connection_count() == 0 {
            return Err(CoreError::Conflict(
                "請先在瀏覽器開啟這份 HTML 簡報，再輸出影片".to_string(),
            ));
        }
        let manifest = context.manifest_repository.snapshot()?;
        let job = self.video_exports.create(
            context.deck_id.clone(),
            deck.format.clone(),
            PathBuf::from(&deck.source_path),
            deck.bridge_path.as_deref().map(PathBuf::from),
            deck.slides.clone(),
            manifest,
            context.audio_storage.clone(),
            &context.presentation_root,
        )?;
        if job.format == "html" {
            self.slide_sync_hub.publish(
                "video_capture_requested",
                json!({
                    "job_id": job.job_id,
                    "bridge_path": job.bridge_path.as_ref().map(|path| path.to_string_lossy()),
                    "slide_ids": job.slides.iter().map(|slide| &slide.slide_id).collect::<Vec<_>>(),
                }),
            );
        } else {
            let app = self.clone();
            let job_id = job.job_id.clone();
            std::thread::Builder::new()
                .name("saislidtake-video-export".to_string())
                .spawn(move || {
                    if let Err(error) = app.finish_video_export(&job_id) {
                        app.video_exports.mark_failed(&job_id, error.to_string());
                    }
                })
                .map_err(CoreError::Io)?;
        }
        self.video_exports.status(&job.job_id)
    }

    pub fn record_video_slide(
        &self,
        job_id: &str,
        slide_index: usize,
        png: &[u8],
    ) -> CoreResult<VideoExportStatus> {
        self.video_exports.record_slide(job_id, slide_index, png)
    }

    pub fn finish_video_export(&self, job_id: &str) -> CoreResult<VideoExportStatus> {
        let initial = self.video_exports.get_job(job_id)?;
        if initial.format == "html" {
            if self.video_exports.begin_processing(job_id)?.is_none() {
                return self.video_exports.status(job_id);
            }
        }
        let job = self.video_exports.get_job(job_id)?;
        let active = self.deck_context()?;
        if active.deck_id != job.deck_id {
            return Err(CoreError::Conflict(
                "影片輸出期間切換了簡報，請重新輸出".to_string(),
            ));
        }
        match render_video(&job, |count| {
            if job.format == "pptx" && count == job.slides.len() {
                self.video_exports.mark_slides_ready(job_id)
            } else {
                Ok(())
            }
        }) {
            Ok((filename, _master)) => self.video_exports.mark_completed(job_id, filename)?,
            Err(error) => {
                self.video_exports.mark_failed(job_id, error.to_string());
                return Err(error);
            }
        }
        self.video_exports.status(job_id)
    }

    pub fn fail_video_export(&self, job_id: &str, message: String) {
        self.video_exports.mark_failed(job_id, message);
    }

    pub fn video_export_status(&self, job_id: &str) -> CoreResult<VideoExportStatus> {
        self.video_exports.status(job_id)
    }

    pub fn video_path(&self, filename: &str) -> CoreResult<PathBuf> {
        self.video_exports.path_for_filename(filename)
    }

    pub fn start_recording(&self, slide_id: &str) -> CoreResult<RecordingStatus> {
        validate_slide_id(slide_id)?;
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| CoreError::Conflict("錄音寫入狀態鎖已失效".to_string()))?;
        let context = self.deck_context()?;
        if let Some(deck) = context.parsed_deck.as_ref() {
            if !deck.slides.iter().any(|slide| slide.slide_id == slide_id) {
                return Err(CoreError::NotFound("找不到指定的簡報頁面".to_string()));
            }
        }
        self.recording_service.start(slide_id)
    }

    pub fn recording_status(&self) -> CoreResult<RecordingStatus> {
        self.recording_service.status()
    }

    pub fn stop_recording(&self) -> CoreResult<SlideTakeGroup> {
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| CoreError::Conflict("錄音寫入狀態鎖已失效".to_string()))?;
        let capture = self.recording_service.stop()?;
        let group = self.save_take_locked(
            &capture.slide_id,
            Some(&capture.recorded_at),
            capture.duration_sec,
            "wav",
            &capture.wav_bytes,
        )?;
        if let Some(warning) = capture.warning {
            self.slide_sync_hub
                .publish("recording_warning", json!({"message": warning}));
        }
        Ok(group)
    }

    pub fn set_deck_active_slide(&self, deck_id: &str, slide_index: usize) -> CoreResult<()> {
        let context = self.deck_context()?;
        if context.deck_id != deck_id {
            return Ok(());
        }
        let Some(deck) = context.parsed_deck.as_ref() else {
            return Ok(());
        };
        let Some(slide) = deck.slides.get(slide_index) else {
            return Ok(());
        };
        self.set_active_slide(ActiveSlideInfo {
            slide_id: slide.slide_id.clone(),
            slide_index,
            total_slides: deck.slides.len(),
            slide_title: slide.title.clone(),
        })
    }

    pub fn active_slide(&self) -> Option<ActiveSlideInfo> {
        self.active_slide
            .read()
            .ok()
            .and_then(|slide| slide.clone())
    }

    pub fn set_active_slide(&self, slide: ActiveSlideInfo) -> CoreResult<()> {
        validate_slide_id(&slide.slide_id)?;
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| CoreError::Conflict("錄音寫入狀態鎖已失效".to_string()))?;
        if let Some(deck) = self.deck_context()?.parsed_deck.as_ref() {
            if deck
                .slides
                .get(slide.slide_index)
                .is_none_or(|expected| expected.slide_id != slide.slide_id)
            {
                return Err(CoreError::Conflict(
                    "目前頁面不符合已載入簡報的頁序".to_string(),
                ));
            }
        }
        if slide.total_slides == 0
            || slide.slide_index >= slide.total_slides
            || slide.slide_title.trim().is_empty()
            || slide.slide_title.len() > 1024
        {
            return Err(CoreError::Invalid("投影片頁碼或標題不合法".to_string()));
        }
        if let Some(capture) = self.recording_service.split_for_slide(&slide.slide_id)? {
            self.save_take_locked(
                &capture.slide_id,
                Some(&capture.recorded_at),
                capture.duration_sec,
                "wav",
                &capture.wav_bytes,
            )?;
            if let Some(warning) = capture.warning {
                self.slide_sync_hub
                    .publish("recording_warning", json!({"message": warning}));
            }
        }
        *self
            .active_slide
            .write()
            .map_err(|_| CoreError::Conflict("目前頁面狀態鎖已失效".to_string()))? =
            Some(slide.clone());
        self.slide_sync_hub.publish("slide_changed", json!(slide));
        Ok(())
    }

    pub fn manifest(&self) -> CoreResult<PresentationManifest> {
        self.deck_context()?.manifest_repository.snapshot()
    }

    pub fn slide_takes(&self, slide_id: &str) -> CoreResult<SlideTakeGroup> {
        validate_slide_id(slide_id)?;
        Ok(self
            .deck_context()?
            .manifest_repository
            .group(slide_id)?
            .unwrap_or_else(|| SlideTakeGroup {
                slide_id: slide_id.to_string(),
                slide_title: self
                    .active_slide()
                    .filter(|slide| slide.slide_id == slide_id)
                    .map(|slide| slide.slide_title)
                    .unwrap_or_else(|| slide_id.to_string()),
                golden_take_id: None,
                next_take_number: 1,
                takes: Vec::new(),
            }))
    }

    pub fn save_take(
        &self,
        slide_id: &str,
        duration_sec: f64,
        extension: &str,
        bytes: &[u8],
    ) -> CoreResult<SlideTakeGroup> {
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| CoreError::Conflict("錄音寫入狀態鎖已失效".to_string()))?;
        self.save_take_locked(slide_id, None, duration_sec, extension, bytes)
    }

    fn save_take_locked(
        &self,
        slide_id: &str,
        recorded_at: Option<&str>,
        duration_sec: f64,
        extension: &str,
        bytes: &[u8],
    ) -> CoreResult<SlideTakeGroup> {
        validate_slide_id(slide_id)?;
        if bytes.is_empty() {
            return Err(CoreError::Invalid("錄音內容不可為空".to_string()));
        }
        if !duration_sec.is_finite() || !(0.0..=86_400.0).contains(&duration_sec) {
            return Err(CoreError::Invalid(
                "錄音秒數必須介於 0 至 86400 秒".to_string(),
            ));
        }
        let context = self.deck_context()?;
        let existing_group = context.manifest_repository.group(slide_id)?;
        let manifest_next = existing_group
            .as_ref()
            .map(|group| {
                group
                    .takes
                    .iter()
                    .filter_map(|take| take.take_id.strip_prefix("take_")?.parse::<u64>().ok())
                    .max()
                    .unwrap_or(0)
                    .saturating_add(1)
                    .max(group.next_take_number.max(1))
            })
            .unwrap_or(1);
        let version = manifest_next.max(context.audio_storage.next_disk_version(slide_id)?);
        let (version, filename, file_path) = context
            .audio_storage
            .write_new_take(slide_id, version, extension, bytes)?;
        let take_id = format!("take_{version}");
        let now = recorded_at
            .map(str::to_string)
            .unwrap_or_else(AudioStorageService::now_created_at);
        let active_title = self
            .active_slide()
            .filter(|slide| slide.slide_id == slide_id)
            .map(|slide| slide.slide_title);

        let result = context.manifest_repository.update(|manifest| {
            let group = manifest
                .slides
                .entry(slide_id.to_string())
                .or_insert_with(|| SlideTakeGroup {
                    slide_id: slide_id.to_string(),
                    slide_title: active_title.clone().unwrap_or_else(|| slide_id.to_string()),
                    golden_take_id: None,
                    next_take_number: version,
                    takes: Vec::new(),
                });
            if let Some(title) = &active_title {
                group.slide_title = title.clone();
            }
            let card = TakeCard {
                take_id: take_id.clone(),
                slide_id: slide_id.to_string(),
                filename: filename.clone(),
                relative_path: format!("audio/{slide_id}/{filename}"),
                duration_sec,
                created_at: now.clone(),
                note: String::new(),
                is_golden: group.golden_take_id.is_none(),
            };
            if group.golden_take_id.is_none() {
                group.golden_take_id = Some(take_id.clone());
            }
            group.takes.push(card);
            group.next_take_number = version
                .checked_add(1)
                .ok_or_else(|| CoreError::Conflict("Take 版本編號已超出可用範圍".to_string()))?;
            for take in &mut group.takes {
                take.is_golden = group.golden_take_id.as_deref() == Some(take.take_id.as_str());
            }
            Ok(group.clone())
        });
        match result {
            Ok(group) => {
                self.slide_sync_hub.publish("takes_updated", json!(group));
                Ok(group)
            }
            Err(error) => {
                let _ = std::fs::remove_file(file_path);
                Err(error)
            }
        }
    }

    pub fn set_golden_take(&self, slide_id: &str, take_id: &str) -> CoreResult<SlideTakeGroup> {
        validate_slide_id(slide_id)?;
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| CoreError::Conflict("錄音寫入狀態鎖已失效".to_string()))?;
        let context = self.deck_context()?;
        let group = context.manifest_repository.update(|manifest| {
            let group = manifest
                .slides
                .get_mut(slide_id)
                .ok_or_else(|| CoreError::NotFound("找不到此投影片的錄音版本".to_string()))?;
            if !group.takes.iter().any(|take| take.take_id == take_id) {
                return Err(CoreError::NotFound("找不到指定的 Take 版本".to_string()));
            }
            group.golden_take_id = Some(take_id.to_string());
            for take in &mut group.takes {
                take.is_golden = take.take_id == take_id;
            }
            Ok(group.clone())
        })?;
        self.slide_sync_hub
            .publish("golden_take_updated", json!(group));
        Ok(group)
    }

    pub fn update_note(
        &self,
        slide_id: &str,
        take_id: &str,
        note: &str,
    ) -> CoreResult<SlideTakeGroup> {
        validate_slide_id(slide_id)?;
        if note.len() > 4096 {
            return Err(CoreError::Invalid("備註不可超過 4096 bytes".to_string()));
        }
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| CoreError::Conflict("錄音寫入狀態鎖已失效".to_string()))?;
        let context = self.deck_context()?;
        let group = context.manifest_repository.update(|manifest| {
            let group = manifest
                .slides
                .get_mut(slide_id)
                .ok_or_else(|| CoreError::NotFound("找不到此投影片的錄音版本".to_string()))?;
            let take = group
                .takes
                .iter_mut()
                .find(|take| take.take_id == take_id)
                .ok_or_else(|| CoreError::NotFound("找不到指定的 Take 版本".to_string()))?;
            take.note = note.to_string();
            Ok(group.clone())
        })?;
        self.slide_sync_hub.publish("takes_updated", json!(group));
        Ok(group)
    }

    pub fn delete_take(&self, slide_id: &str, take_id: &str) -> CoreResult<SlideTakeGroup> {
        validate_slide_id(slide_id)?;
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| CoreError::Conflict("錄音寫入狀態鎖已失效".to_string()))?;
        let context = self.deck_context()?;
        let group_before = context
            .manifest_repository
            .group(slide_id)?
            .ok_or_else(|| CoreError::NotFound("找不到此投影片的錄音版本".to_string()))?;
        let take = group_before
            .takes
            .iter()
            .find(|take| take.take_id == take_id)
            .cloned()
            .ok_or_else(|| CoreError::NotFound("找不到指定的 Take 版本".to_string()))?;
        let tombstone = context
            .audio_storage
            .move_to_tombstone(slide_id, &take.filename)?;
        let result = context.manifest_repository.update(|manifest| {
            let group = manifest
                .slides
                .get_mut(slide_id)
                .ok_or_else(|| CoreError::NotFound("找不到此投影片的錄音版本".to_string()))?;
            group.takes.retain(|candidate| candidate.take_id != take_id);
            if group.golden_take_id.as_deref() == Some(take_id) {
                group.golden_take_id = None;
            }
            for candidate in &mut group.takes {
                candidate.is_golden =
                    group.golden_take_id.as_deref() == Some(candidate.take_id.as_str());
            }
            Ok(group.clone())
        });
        match result {
            Ok(group) => {
                let _ = context.audio_storage.finish_delete(&tombstone);
                self.slide_sync_hub.publish("take_deleted", json!(group));
                Ok(group)
            }
            Err(error) => {
                let _ =
                    context
                        .audio_storage
                        .restore_tombstone(slide_id, &take.filename, &tombstone);
                Err(error)
            }
        }
    }
}

fn stable_deck_id(stem: &str, canonical_path: &str) -> String {
    let slug = stem
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else if character.is_whitespace() {
                '_'
            } else {
                '-'
            }
        })
        .take(48)
        .collect::<String>()
        .trim_matches(['-', '_'])
        .to_ascii_lowercase();
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in canonical_path.as_bytes() {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
    }
    format!(
        "deck_{}_{}",
        if slug.is_empty() {
            "presentation"
        } else {
            &slug
        },
        format_args!("{hash:016x}")
    )
}

fn persist_deck_info(root: &Path, deck_id: &str, deck: &ParsedDeck) -> CoreResult<()> {
    let path = root.join("deck.json");
    let mut temporary = NamedTempFile::new_in(root)?;
    serde_json::to_writer_pretty(&mut temporary, &json!({"deck_id": deck_id, "deck": deck}))?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .map_err(|error| CoreError::Io(error.error))?;
    Ok(())
}

fn inject_bridge_sidecar(
    source_path: &Path,
    profile: Option<&DeckParserProfile>,
) -> CoreResult<PathBuf> {
    const MARKER: &str = "<!-- SAISlideTake auto bridge -->";
    const SCRIPT: &str = "<script src=\"http://127.0.0.1:3188/sai-slidetake-bridge.js\"></script>";
    let source = fs::read_to_string(source_path)
        .map_err(|error| CoreError::Invalid(format!("無法讀取 HTML 簡報：{error}")))?;
    if source.contains("sai-slidetake-bridge.js") && profile.is_none() {
        return Ok(source_path.to_path_buf());
    }
    let lower = source.to_ascii_lowercase();
    let profile_script = profile
        .map(serde_json::to_string)
        .transpose()?
        .map(|json| {
            format!(
                "<script>window.__SAISLIDETAKE_PARSER_PROFILE__={};</script>\n",
                json.replace('<', "\\u003c")
            )
        })
        .unwrap_or_default();
    let mut injected = source.clone();
    if !source.contains("sai-slidetake-bridge.js") {
        let insertion = lower
            .rfind("</body>")
            .or_else(|| lower.rfind("</html>"))
            .unwrap_or(source.len());
        let bridge_tag = format!("{}{}\n{}", profile_script, MARKER, SCRIPT);
        injected = format!(
            "{}{}{}",
            &source[..insertion],
            bridge_tag,
            &source[insertion..]
        );
    } else if profile.is_some() {
        let bridge_source = lower
            .find("sai-slidetake-bridge.js")
            .unwrap_or(source.len());
        let script_start = lower[..bridge_source].rfind("<script");
        let insertion = script_start.or_else(|| lower.rfind("</head>")).unwrap_or(0);
        injected = format!(
            "{}{}{}{}",
            &source[..insertion],
            MARKER,
            profile_script,
            &source[insertion..]
        );
    }
    let parent = source_path
        .parent()
        .ok_or_else(|| CoreError::Invalid("HTML 簡報缺少所在目錄".to_string()))?;
    let stem = source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("presentation");
    let extension = source_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("html");
    let mut sequence = 1_u32;
    loop {
        let suffix = if sequence == 1 {
            "saislidtake".to_string()
        } else {
            format!("saislidtake-{sequence}")
        };
        let target = parent.join(format!("{stem}.{suffix}.{extension}"));
        if target.exists() {
            let existing = fs::read_to_string(&target).unwrap_or_default();
            if !existing.contains(MARKER) {
                sequence += 1;
                continue;
            }
        }
        let mut temporary = NamedTempFile::new_in(parent)?;
        temporary.write_all(injected.as_bytes())?;
        temporary.as_file().sync_all()?;
        temporary
            .persist(&target)
            .map_err(|error| CoreError::Io(error.error))?;
        return Ok(target);
    }
}
