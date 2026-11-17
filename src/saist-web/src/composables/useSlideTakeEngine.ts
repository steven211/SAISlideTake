import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import type { BridgeEvent, DeckInfo, TakeCard, TakeGroup, PresentationManifest } from '../types';
import { useLocale } from '../locales';
import { useStudioSettings } from './useStudioSettings';
import { useDeckHistory } from './useDeckHistory';

const API = 'http://127.0.0.1:3188';
const WS = 'ws://127.0.0.1:3188/ws';

export function useSlideTakeEngine() {
  const currentSlideId = ref('');
  const currentSlideIndex = ref(0);
  const totalSlides = ref(0);
  const slideTitle = ref('');
  const group = ref<TakeGroup | null>(null);
  const connection = ref<'connecting' | 'online' | 'offline'>('connecting');
  const isRecording = ref(false);
  const recordingSeconds = ref(0);
  const waveform = ref<number[]>(Array(14).fill(0.08));
  const isAutoplaying = ref(false);
  const drawerOpen = ref(false);
  const playingTakeId = ref<string | null>(null);
  const playingSlideId = ref<string | null>(null);
  const isPreviewPaused = ref(true);
  const playbackPosition = ref(0);
  const playbackDuration = ref(0);
  const errorMessage = ref('');
  const deck = ref<DeckInfo | null>(null);
  const manifest = ref<PresentationManifest | null>(null);
  const videoExporting = ref(false);
  const videoExportProgress = ref('');
  const selectedSlideId = ref('');
  const studioOpen = ref(false);
  const isDraggingOver = ref(false);
  const pendingDeleteTake = ref<{ takeId: string; slideId: string } | null>(null);
  const takes = computed(() => [...(group.value?.takes ?? [])].sort((a, b) => b.created_at.localeCompare(a.created_at)));
  const goldenTakeId = computed(() => group.value?.golden_take_id ?? null);
  const { t } = useLocale();
  const { parserProfileForIngest } = useStudioSettings();
  const { addDeckToHistory } = useDeckHistory();

  watch(deck, (loadedDeck) => {
    if (loadedDeck) addDeckToHistory(loadedDeck);
  }, { immediate: true });

  let socket: WebSocket | null = null;
  let reconnectTimer = 0;
  let animationFrame = 0;
  let recordingStatusTimer = 0;
  let recordingStartedAt = 0;
  let recordingSlideId = '';
  let previewAudio: HTMLAudioElement | null = null;
  let disposed = false;
  let embeddedPresentationActive = false;
  let dropUnlisteners: Array<() => void> = [];

  function getNativeWindow() {
    type NativeWindow = { show: () => Promise<void>; hide: () => Promise<void> };
    const tauri = (window as Window & { __TAURI__?: { window?: { getCurrentWindow: () => NativeWindow } } }).__TAURI__;
    return tauri?.window?.getCurrentWindow();
  }

  function showError(message: string) {
    errorMessage.value = message;
    window.setTimeout(() => { if (errorMessage.value === message) errorMessage.value = ''; }, 4000);
  }

  async function request<T>(path: string, init?: RequestInit): Promise<T> {
    const response = await fetch(`${API}${path}`, init);
    if (!response.ok) {
      const payload = await response.json().catch(() => ({}));
      throw new Error(payload.error || `HTTP ${response.status}`);
    }
    return response.json() as Promise<T>;
  }

  async function refreshTakes(slideId = selectedSlideId.value || currentSlideId.value) {
    if (!slideId) { group.value = null; return; }
    try {
      group.value = await request<TakeGroup>(`/api/slides/${encodeURIComponent(slideId)}/takes`);
    } catch { /* Keep the last cards visible during a brief service reconnect. */ }
  }

  function handleEvent(event: BridgeEvent) {
    if (event.type === 'deck_loaded' && event.payload && typeof event.payload === 'object') {
      deck.value = event.payload as DeckInfo;
      selectedSlideId.value = deck.value.slides.some((slide) => slide.slide_id === selectedSlideId.value)
        ? selectedSlideId.value : deck.value.slides[0]?.slide_id ?? '';
      void refreshManifest();
      void refreshTakes();
    } else if (event.type === 'slide_changed' && event.payload && typeof event.payload === 'object') {
      const slide = event.payload as { slide_id: string; slide_index: number; total_slides: number; slide_title: string };
      if (isRecording.value) recordingSlideId = slide.slide_id;
      currentSlideId.value = slide.slide_id;
      currentSlideIndex.value = slide.slide_index;
      totalSlides.value = slide.total_slides;
      slideTitle.value = slide.slide_title;
      if (!studioOpen.value) selectedSlideId.value = slide.slide_id;
      void refreshTakes(studioOpen.value ? selectedSlideId.value : slide.slide_id);
    } else if (['takes_updated', 'golden_take_updated', 'take_deleted'].includes(event.type)) {
      const changed = event.payload as TakeGroup | undefined;
      if (changed?.slide_id === selectedSlideId.value || changed?.slide_id === currentSlideId.value) {
        if (changed.slide_id === selectedSlideId.value) group.value = changed;
        if (manifest.value) manifest.value.slides[changed.slide_id] = changed;
      }
    } else if (event.type === 'start_golden_autoplay') {
      isAutoplaying.value = true;
    } else if (event.type === 'stop_golden_autoplay') {
      isAutoplaying.value = false;
    } else if (event.type === 'embedded_ui_attached' || event.type === 'embedded_ui_detached') {
      const count = Number((event.payload as { count?: number } | undefined)?.count ?? 0);
      embeddedPresentationActive = count > 0;
      if (!studioOpen.value) {
        if (embeddedPresentationActive) {
          void getNativeWindow()?.hide().catch(() => {});
        } else {
          void setWindowMode('capsule').then(() => getNativeWindow()?.show()).catch(() => {});
        }
      }
    } else if (event.type === 'toggle_native_overlay') {
      const visible = Boolean((event.payload as { visible?: boolean } | undefined)?.visible);
      if (visible) void setWindowMode('capsule').then(() => getNativeWindow()?.show()).catch(() => {});
      else void getNativeWindow()?.hide().catch(() => {});
    } else if (event.type === 'open_native_studio') {
      toggleStudio(true);
    } else if (event.type === 'keyboard_shortcut') {
      const key = (event.payload as { key?: string } | undefined)?.key?.toLowerCase();
      if (key === 'r') void toggleRecording();
      if (key === 'space') void replayGolden();
    } else if (event.type === 'autoplay_finished' || event.type === 'autoplay_error') {
      isAutoplaying.value = false;
      if (event.type === 'autoplay_error') showError(t('saveFailed'));
    } else if (event.type === 'recording_warning') {
      const warning = (event.payload as { message?: string } | undefined)?.message;
      if (warning) showError(warning);
    }
  }

  function connect() {
    if (disposed) return;
    connection.value = 'connecting';
    try {
      socket = new WebSocket(WS);
      socket.onopen = () => { connection.value = 'online'; };
      socket.onmessage = (message) => {
        try { handleEvent(JSON.parse(message.data) as BridgeEvent); } catch { /* Ignore malformed local events. */ }
      };
      socket.onerror = () => socket?.close();
      socket.onclose = () => {
        connection.value = 'offline';
        if (!disposed) reconnectTimer = window.setTimeout(connect, 1200);
      };
    } catch {
      connection.value = 'offline';
      reconnectTimer = window.setTimeout(connect, 1200);
    }
  }

  function sendEvent(type: string, payload: unknown = {}) {
    if (socket?.readyState !== WebSocket.OPEN) {
      showError(t('connectionOffline'));
      return false;
    }
    socket.send(JSON.stringify({ type, payload }));
    return true;
  }

  function resizeOverlay(expanded: boolean) {
    const tauri = (window as Window & { __TAURI__?: { core?: { invoke: (command: string, args: object) => Promise<unknown> } } }).__TAURI__;
    void tauri?.core?.invoke('resize_overlay', { mode: expanded ? 'drawer' : 'capsule' }).catch(() => {});
  }

  function setWindowMode(mode: 'capsule' | 'drawer' | 'studio'): Promise<void> {
    const tauri = (window as Window & { __TAURI__?: { core?: { invoke: (command: string, args: object) => Promise<unknown> } } }).__TAURI__;
    const invoke = tauri?.core?.invoke;
    if (!invoke) {
      showError(t('connectionOffline'));
      return Promise.resolve();
    }
    return invoke('resize_overlay', { mode }).then(() => undefined).catch((error: unknown) => {
      showError(error instanceof Error ? error.message : String(error));
    });
  }

  function toggleDrawer(force?: boolean) {
    if (studioOpen.value) return;
    drawerOpen.value = force ?? !drawerOpen.value;
    resizeOverlay(drawerOpen.value);
    if (drawerOpen.value) void refreshTakes();
  }

  function toggleStudio(force?: boolean) {
    studioOpen.value = force ?? !studioOpen.value;
    if (studioOpen.value) {
      drawerOpen.value = false;
      void setWindowMode('studio').then(() => getNativeWindow()?.show()).catch(() => {});
      void refreshDeck();
      void refreshManifest();
      void refreshTakes();
    } else {
      void setWindowMode('capsule').then(() => {
        if (embeddedPresentationActive) return getNativeWindow()?.hide();
        return getNativeWindow()?.show();
      });
    }
  }

  function selectStudioSlide(slideId: string) {
    if (!deck.value?.slides.some((slide) => slide.slide_id === slideId)) return;
    selectedSlideId.value = slideId;
    void refreshTakes(slideId);
  }

  async function refreshManifest() {
    try { manifest.value = await request<PresentationManifest>('/api/manifest'); } catch { /* Retry on the next deck event. */ }
  }

  async function refreshDeck() {
    try {
      deck.value = await request<DeckInfo | null>('/api/deck');
      if (!deck.value) return;
      const slides = deck.value.slides;
      const preferred = currentSlideId.value || selectedSlideId.value;
      selectedSlideId.value = slides.some((slide) => slide.slide_id === preferred)
        ? preferred : slides[0]?.slide_id ?? '';
      if (selectedSlideId.value) void refreshTakes(selectedSlideId.value);
    } catch { /* Core reconnect will retry on mount. */ }
  }

  async function ingestDeck(filePath: string) {
    try {
      const parserProfile = parserProfileForIngest();
      deck.value = await request<DeckInfo>('/api/deck/ingest', {
        method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ file_path: filePath, parser_profile: parserProfile }),
      });
      selectedSlideId.value = deck.value.slides[0]?.slide_id ?? '';
      group.value = null;
      await Promise.all([refreshManifest(), refreshTakes()]);
      if (deck.value.format === 'html' && deck.value.bridge_path) {
        const tauri = (window as Window & { __TAURI__?: { core?: { invoke: (command: string, args: object) => Promise<unknown> } } }).__TAURI__;
        await tauri?.core?.invoke('open_html_presentation', { path: deck.value.bridge_path });
      }
      showError(t('deckLoaded'));
    } catch (error) {
      showError(error instanceof Error ? error.message : t('deckLoadFailed'));
    }
  }

  async function exportMasterAudio() {
    try {
      const result = await request<{ filename: string }>('/api/deck/export-master-audio', { method: 'POST' });
      const response = await fetch(`${API}/api/deck/master-audio/${encodeURIComponent(result.filename)}`);
      if (!response.ok) throw new Error(t('masterExportFailed'));
      const objectUrl = URL.createObjectURL(await response.blob());
      const link = document.createElement('a');
      link.href = objectUrl;
      link.download = result.filename;
      link.click();
      URL.revokeObjectURL(objectUrl);
      showError(t('masterExported'));
    } catch (error) {
      showError(error instanceof Error ? error.message : t('masterExportFailed'));
    }
  }

  async function exportVideo() {
    if (videoExporting.value) return;
    videoExporting.value = true;
    try {
      videoExportProgress.value = '';
      const job = await request<{ job_id: string }>('/api/deck/export-video', { method: 'POST' });
      while (!disposed) {
        const status = await request<{ status: string; filename?: string; error?: string; captured_slides?: number; total_slides?: number }>(
          `/api/deck/video-export/${encodeURIComponent(job.job_id)}`,
        );
        if (status.total_slides) videoExportProgress.value = `${status.captured_slides ?? 0}/${status.total_slides}`;
        if (status.status === 'completed' && status.filename) {
          const response = await fetch(`${API}/api/deck/video/${encodeURIComponent(status.filename)}`);
          if (!response.ok) throw new Error(t('videoExportFailed'));
          const objectUrl = URL.createObjectURL(await response.blob());
          const link = document.createElement('a');
          link.href = objectUrl;
          link.download = status.filename;
          link.click();
          window.setTimeout(() => URL.revokeObjectURL(objectUrl), 1000);
          showError(t('videoExported'));
          return;
        }
        if (status.status === 'failed') throw new Error(status.error || t('videoExportFailed'));
        await new Promise((resolve) => window.setTimeout(resolve, 600));
      }
    } catch (error) {
      showError(error instanceof Error ? error.message : t('videoExportFailed'));
    } finally {
      videoExporting.value = false;
      videoExportProgress.value = '';
    }
  }

  function exitApplication() {
    const tauri = (window as Window & { __TAURI__?: { core?: { invoke: (command: string, args: object) => Promise<unknown> } } }).__TAURI__;
    void tauri?.core?.invoke('exit_application', {}).catch((error: unknown) => {
      showError(error instanceof Error ? error.message : t('saveFailed'));
    });
  }

  async function installFileDrop() {
    const tauri = (window as Window & { __TAURI__?: { webview?: { getCurrentWebview: () => { onDragDropEvent: (callback: (event: { payload: { type: string; paths?: string[] } }) => void) => Promise<() => void> } } } }).__TAURI__;
    const webview = tauri?.webview?.getCurrentWebview();
    if (!webview) return;
    const unlisten = await webview.onDragDropEvent((event) => {
      const type = event.payload.type;
      isDraggingOver.value = type === 'enter' || type === 'over';
      if (type === 'drop') {
        const path = event.payload.paths?.find((filePath) => /\.(html?|pptx)$/i.test(filePath)) ?? '';
        if (path) void ingestDeck(path);
        else showError(t('deckLoadFailed'));
      }
    });
    dropUnlisteners = [unlisten];
  }

  function onNativeDragOver(event: DragEvent) { event.preventDefault(); isDraggingOver.value = true; }
  function onNativeDragLeave() { isDraggingOver.value = false; }

  async function startRecording() {
    const slideId = currentSlideId.value || selectedSlideId.value;
    if (!slideId) { showError(t('noActiveSlide')); return; }
    try {
      await request('/api/recording/start', {
        method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ slide_id: slideId }),
      });
      recordingSlideId = slideId;
      recordingStartedAt = Date.now();
      isRecording.value = true;
      tickRecording();
      recordingStatusTimer = window.setInterval(() => { void updateRecordingLevel(); }, 180);
    } catch (error) {
      showError(error instanceof Error ? error.message : t('microphoneError'));
    }
  }

  async function updateRecordingLevel() {
    try {
      const status = await request<{ active: boolean; elapsed_sec: number; level: number; error?: string }>('/api/recording/status');
      if (status.error) { showError(status.error); stopRecording(); return; }
      if (!status.active) {
        isRecording.value = false;
        cleanupRecorder();
        void refreshTakes(recordingSlideId);
        return;
      }
      waveform.value = Array.from({ length: 14 }, () => Math.max(0.08, Math.min(1, status.level * (0.45 + Math.random() * 1.1))));
    } catch { /* The stop request will report a service failure if the core disconnected. */ }
  }

  function tickRecording() {
    if (!isRecording.value) return;
    recordingSeconds.value = Math.floor((Date.now() - recordingStartedAt) / 1000);
    animationFrame = requestAnimationFrame(tickRecording);
  }

  function stopRecording() {
    if (!isRecording.value) return;
    isRecording.value = false;
    cancelAnimationFrame(animationFrame);
    window.clearInterval(recordingStatusTimer);
    void request<TakeGroup>('/api/recording/stop', { method: 'POST' }).then((savedGroup) => {
      if (selectedSlideId.value === recordingSlideId || currentSlideId.value === recordingSlideId) group.value = savedGroup;
      if (manifest.value) manifest.value.slides[recordingSlideId] = savedGroup;
      showError(t('recordingSaved'));
      cleanupRecorder();
    }).catch((error) => {
      cleanupRecorder();
      showError(error instanceof Error ? error.message : t('saveFailed'));
    });
  }

  function cleanupRecorder() {
    cancelAnimationFrame(animationFrame);
    window.clearInterval(recordingStatusTimer);
    isRecording.value = false;
    recordingSeconds.value = 0;
    waveform.value = Array(14).fill(0.08);
  }

  function toggleRecording() {
    if (isRecording.value) stopRecording();
    else void startRecording();
  }

  function stopRecordingOnExit() {
    if (!isRecording.value) return;
    isRecording.value = false;
    window.clearInterval(recordingStatusTimer);
    void fetch(`${API}/api/recording/stop`, { method: 'POST', keepalive: true }).catch(() => {});
  }

  async function playTake(take: TakeCard) {
    if (playingSlideId.value === take.slide_id && playingTakeId.value === take.take_id && previewAudio) {
      if (previewAudio.paused) {
        try { await previewAudio.play(); } catch { showError(t('saveFailed')); }
      } else {
        previewAudio.pause();
      }
      return;
    }
    previewAudio?.pause();
    const audio = new Audio(`${API}/api/audio/${encodeURIComponent(take.slide_id)}/${encodeURIComponent(take.filename)}`);
    previewAudio = audio;
    playingSlideId.value = take.slide_id;
    playingTakeId.value = take.take_id;
    isPreviewPaused.value = false;
    playbackPosition.value = 0;
    playbackDuration.value = take.duration_sec;
    audio.onloadedmetadata = () => {
      if (previewAudio === audio && Number.isFinite(audio.duration)) playbackDuration.value = audio.duration;
    };
    audio.ontimeupdate = () => { if (previewAudio === audio) playbackPosition.value = audio.currentTime; };
    audio.onplay = () => { if (previewAudio === audio) isPreviewPaused.value = false; };
    audio.onpause = () => { if (previewAudio === audio) isPreviewPaused.value = true; };
    audio.onended = () => {
      if (previewAudio === audio) {
        playingSlideId.value = null;
        playingTakeId.value = null;
        isPreviewPaused.value = true;
        playbackPosition.value = 0;
      }
    };
    audio.onerror = () => {
      if (previewAudio === audio) {
        playingSlideId.value = null;
        playingTakeId.value = null;
        isPreviewPaused.value = true;
      }
      showError(t('saveFailed'));
    };
    try { await audio.play(); } catch {
      if (previewAudio === audio) {
        playingSlideId.value = null;
        playingTakeId.value = null;
        isPreviewPaused.value = true;
      }
      showError(t('saveFailed'));
    }
  }

  function seekTake(takeId: string, seconds: number, slideId?: string) {
    if (playingTakeId.value !== takeId || (slideId && playingSlideId.value !== slideId) || !previewAudio || !Number.isFinite(seconds)) return;
    previewAudio.currentTime = Math.max(0, Math.min(seconds, previewAudio.duration || playbackDuration.value));
    playbackPosition.value = previewAudio.currentTime;
  }

  function replayGolden() {
    const take = group.value?.takes.find((item) => item.take_id === group.value?.golden_take_id);
    if (take) void playTake(take);
    else showError(t('emptyTakes'));
  }

  function toggleAutoplay() {
    if (isAutoplaying.value) {
      sendEvent('stop_golden_autoplay');
      isAutoplaying.value = false;
    } else if (sendEvent('start_golden_autoplay', { start_slide_index: 0, bridge_path: deck.value?.bridge_path })) {
      isAutoplaying.value = true;
    }
  }

  function autoplayTakeColumn(slot: number) {
    if (!Number.isInteger(slot) || slot < 1 || slot > 5) return;
    if (sendEvent('start_golden_autoplay', { start_slide_index: 0, take_slot: slot, bridge_path: deck.value?.bridge_path })) {
      isAutoplaying.value = true;
    }
  }

  async function setGolden(takeId: string, slideId = selectedSlideId.value || currentSlideId.value) {
    try {
      const updated = await request<TakeGroup>(`/api/slides/${encodeURIComponent(slideId)}/golden`, {
        method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ take_id: takeId }),
      });
      if (selectedSlideId.value === slideId || currentSlideId.value === slideId) group.value = updated;
      if (manifest.value) manifest.value.slides[slideId] = updated;
    } catch { showError(t('saveFailed')); }
  }

  function requestDeleteTake(takeId: string, slideId = selectedSlideId.value || currentSlideId.value) {
    if (!slideId) { showError(t('noActiveSlide')); return; }
    pendingDeleteTake.value = { takeId, slideId };
  }

  function cancelDeleteTake() {
    pendingDeleteTake.value = null;
  }

  async function confirmDeleteTake() {
    const pending = pendingDeleteTake.value;
    if (!pending) return;
    pendingDeleteTake.value = null;
    try {
      const deleted = await request<TakeGroup>(`/api/slides/${encodeURIComponent(pending.slideId)}/takes/${encodeURIComponent(pending.takeId)}`, { method: 'DELETE' });
      if (selectedSlideId.value === pending.slideId || currentSlideId.value === pending.slideId) group.value = deleted;
      if (manifest.value) manifest.value.slides[pending.slideId] = deleted;
      if (playingTakeId.value === pending.takeId && playingSlideId.value === pending.slideId) {
        previewAudio?.pause();
        playingSlideId.value = null;
        playingTakeId.value = null;
        isPreviewPaused.value = true;
        playbackPosition.value = 0;
      }
    } catch (error) { showError(error instanceof Error ? error.message : t('saveFailed')); }
  }

  async function saveNote(takeId: string, note: string) {
    try {
      group.value = await request<TakeGroup>(`/api/slides/${encodeURIComponent(selectedSlideId.value || currentSlideId.value)}/takes/${encodeURIComponent(takeId)}`, {
        method: 'PATCH', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ note }),
      });
    } catch { showError(t('saveFailed')); }
  }

  function onKeyDown(event: KeyboardEvent) {
    if (pendingDeleteTake.value) {
      if (event.key === 'Escape') { event.preventDefault(); cancelDeleteTake(); }
      return;
    }
    const target = event.target as HTMLElement | null;
    if (target && ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName)) return;
    if (event.key.toLowerCase() === 'r') { event.preventDefault(); toggleRecording(); }
    if (event.code === 'Space') { event.preventDefault(); replayGolden(); }
  }

  onMounted(() => {
    connect();
    void installFileDrop();
    void refreshDeck();
    void refreshManifest();
    window.addEventListener('dragover', onNativeDragOver);
    window.addEventListener('dragleave', onNativeDragLeave);
    window.addEventListener('keydown', onKeyDown);
  });

  onUnmounted(() => {
    disposed = true;
    window.clearTimeout(reconnectTimer);
    socket?.close();
    dropUnlisteners.forEach((unlisten) => unlisten());
    window.removeEventListener('dragover', onNativeDragOver);
    window.removeEventListener('dragleave', onNativeDragLeave);
    window.removeEventListener('keydown', onKeyDown);
    stopRecordingOnExit();
    cleanupRecorder();
    previewAudio?.pause();
  });

  return {
    currentSlideIndex, totalSlides, slideTitle, isRecording, recordingSeconds,
    waveform, takes, goldenTakeId, playingTakeId, playingSlideId, isPreviewPaused,
    playbackPosition, playbackDuration, connection, isAutoplaying, videoExporting, videoExportProgress,
    drawerOpen, errorMessage, toggleRecording, replayGolden, toggleAutoplay, autoplayTakeColumn,
    toggleDrawer, playTake, seekTake, setGolden, requestDeleteTake, confirmDeleteTake, cancelDeleteTake, saveNote,
    deck, manifest, selectedSlideId, studioOpen, isDraggingOver, pendingDeleteTake,
    toggleStudio, selectStudioSlide, ingestDeck, exportMasterAudio, exportVideo, exitApplication,
  };
}
