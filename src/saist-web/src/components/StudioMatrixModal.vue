<template>
  <section v-if="open" class="studio-window" role="dialog" :aria-label="t('studio')">
    <header class="studio-header" @mousedown="startWindowDrag">
      <div><small>{{ deck?.format?.toUpperCase() || 'SAISlideTake' }}</small><h1>{{ deck?.title || t('deckEmpty') }}</h1></div>
      <div class="studio-head-actions">
        <span>{{ deck?.slides.length || 0 }} {{ t('slides') }}</span>
        <button class="studio-icon" :class="{ recording: isRecording }" :title="isRecording ? t('stop') : t('record')" @click="$emit('toggle-recording')">{{ isRecording ? '■' : '●' }}</button>
        <div class="studio-window-controls">
          <button class="studio-window-control minimize" :title="t('minimize')" :aria-label="t('minimize')" @mousedown.stop @click.stop="minimizeWindow">−</button>
          <button class="studio-window-control maximize" :title="t(isMaximized ? 'restore' : 'maximize')" :aria-label="t(isMaximized ? 'restore' : 'maximize')" @mousedown.stop @click.stop="toggleMaximize">{{ isMaximized ? '❐' : '□' }}</button>
          <button class="studio-window-control close" :title="t('close')" :aria-label="t('close')" @mousedown.stop @click.stop="$emit('close')">×</button>
        </div>
      </div>
    </header>
    <nav class="studio-tabs" :aria-label="t('studioTabs')">
      <button class="studio-tab" :class="{ active: activeTab === 'studio' }" @click="activeTab = 'studio'">{{ t('recordingStudio') }}</button>
      <button class="studio-tab" :class="{ active: activeTab === 'settings' }" @click="activeTab = 'settings'">{{ t('settings') }}</button>
    </nav>
    <StudioSettingsTab v-if="activeTab === 'settings'" :deck="deck" />
    <div v-else class="studio-content">
      <div class="studio-matrix-main">
        <div class="studio-matrix-header">
          <strong>{{ t('slides') }}</strong>
          <button v-for="slot in slots" :key="slot" class="take-column-play" :disabled="!hasTakeColumn(slot)" :title="t('playTakeColumn').replace('{slot}', String(slot).padStart(2, '0'))" @click="$emit('autoplay-take-column', slot)">
            <span>{{ t('take') }} {{ String(slot).padStart(2, '0') }}</span><span aria-hidden="true">▶</span>
          </button>
        </div>
        <div class="studio-matrix-rows">
          <div v-for="row in rows" :key="row.slide.slide_id" class="studio-row" :class="{ selected: row.slide.slide_id === selectedSlideId }">
            <button class="slide-cell" @click="$emit('select-slide', row.slide.slide_id)">
              <span class="slide-label"><i>{{ String(row.slide.slide_index + 1).padStart(2, '0') }}</i><strong>{{ row.slide.title }}</strong></span>
              <small v-if="row.slide.notes" class="slide-note">{{ row.slide.notes }}</small>
            </button>
            <div v-for="slot in slots" :key="slot" class="take-cell">
              <article v-if="row.takes[slot - 1]" class="mini-take" :class="{ golden: row.takes[slot - 1]?.is_golden }">
                <div class="take-meta"><span>{{ formatDuration(row.takes[slot - 1]!.duration_sec) }}</span><time :title="t('recordedAt')">{{ formatCreatedAt(row.takes[slot - 1]!.created_at) }}</time></div>
                <input v-if="isPlaying(row.slide.slide_id, row.takes[slot - 1]?.take_id)" class="mini-seek" type="range" min="0" :max="playbackDuration || row.takes[slot - 1]!.duration_sec || 1" step="0.1" :value="playbackPosition" @input="seek($event, row.takes[slot - 1]!.take_id, row.slide.slide_id)" />
                <div class="mini-actions">
                  <button :title="t('play')" @click="$emit('play', row.takes[slot - 1])">{{ isPlaying(row.slide.slide_id, row.takes[slot - 1]?.take_id) && !isPreviewPaused ? 'Ⅱ' : '▶' }}</button>
                  <button v-if="!row.takes[slot - 1]?.is_golden" :title="t('makeGolden')" @click="$emit('make-golden', row.takes[slot - 1]!.take_id, row.slide.slide_id)">★</button>
                  <span v-else class="golden-star">★</span>
                  <button class="delete-action" :title="t('delete')" @click="$emit('delete', row.takes[slot - 1]!.take_id, row.slide.slide_id)">×</button>
                </div>
              </article>
              <div v-else class="empty-take">—</div>
            </div>
          </div>
          <p v-if="!rows.length" class="empty-deck">{{ t('unknownSlide') }}</p>
        </div>
      </div>

      <aside class="studio-history-sidebar" :aria-label="t('history')">
        <header class="history-sidebar-header">
          <div class="history-title-wrap">
            <span class="history-badge-icon" aria-hidden="true">⏱</span>
            <strong>{{ t('history') }}</strong>
            <span class="history-badge-count">({{ historyList.length }})</span>
          </div>
          <button v-if="historyList.length" class="history-clear-btn" :title="t('clearHistory')" :aria-label="t('clearHistory')" @click="clearHistory">✕</button>
        </header>
        <div v-if="!historyList.length" class="history-empty">
          <p>{{ t('historyEmpty') }}</p>
          <small>{{ t('doubleClickToImport') }}</small>
        </div>
        <div v-else class="history-list">
          <article
            v-for="item in historyList"
            :key="item.filePath"
            class="history-card"
            :class="{ active: item.filePath === deck?.source_path }"
            :title="`${item.title}\n${item.filePath}\n${t('doubleClickToImport')}`"
            @dblclick="onHistoryDoubleClick(item.filePath)"
          >
            <div class="history-card-top">
              <strong class="history-card-title">{{ item.title }}</strong>
              <span class="history-card-format">{{ item.format.toUpperCase() }}</span>
            </div>
            <div class="history-card-meta">
              <span>{{ item.slidesCount }} {{ t('slides') }}</span>
              <time>{{ formatHistoryTime(item.lastOpenedAt) }}</time>
            </div>
            <div class="history-card-path" :title="item.filePath">{{ item.filePath }}</div>
            <div class="history-card-actions">
              <button class="history-btn-import" :title="t('doubleClickToImport')" @click.stop="onHistoryDoubleClick(item.filePath)">
                <span>{{ t('importDeck') }}</span>
              </button>
              <button class="history-btn-del" :title="t('removeFromHistory')" :aria-label="t('removeFromHistory')" @click.stop="removeHistoryItem(item.filePath)">
                ×
              </button>
            </div>
          </article>
        </div>
      </aside>
    </div>
    <footer v-if="activeTab === 'studio'" class="studio-footer">
      <button class="autoplay-action" :class="{ active: isAutoplaying }" @click="$emit('autoplay')">{{ isAutoplaying ? t('autoplayStop') : t('autoplayAll') }}</button>
      <button class="export-action" @click="$emit('export-master')">{{ t('exportMaster') }}</button>
      <button class="video-action" :disabled="!deck?.slides.length || videoExporting" @click="$emit('export-video')">{{ videoExporting ? `${t('videoExporting')} ${videoExportProgress}` : t('exportVideo') }}</button>
      <button class="exit-action" @click="$emit('exit-app')">{{ t('exitApp') }}</button>
    </footer>
  </section>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import type { DeckInfo, TakeCard, TakeGroup } from '../types';
import { useLocale } from '../locales';
import StudioSettingsTab from './StudioSettingsTab.vue';
import { useDeckHistory } from '../composables/useDeckHistory';

const props = defineProps<{
  open: boolean; deck: DeckInfo | null; manifest: Record<string, TakeGroup> | null;
  selectedSlideId: string; playingTakeId: string | null; playingSlideId?: string | null; isPreviewPaused: boolean;
  playbackPosition: number; playbackDuration: number;
  isRecording: boolean; isAutoplaying: boolean; videoExporting: boolean; videoExportProgress: string;
}>();
const { t } = useLocale();
const { historyList, removeHistoryItem, clearHistory } = useDeckHistory();
const isMaximized = ref(false);
const activeTab = ref<'studio' | 'settings'>('studio');
const slots = [1, 2, 3, 4, 5];
const rows = computed(() => (props.deck?.slides ?? []).map((slide) => ({
  slide,
  takes: slots.map((slot) => (props.manifest?.[slide.slide_id]?.takes ?? []).find((take) => takeNumber(take) === slot) ?? null),
})));
function isPlaying(slideId: string, takeId?: string) {
  return Boolean(takeId && props.playingSlideId === slideId && props.playingTakeId === takeId);
}
function takeNumber(take: TakeCard) { return Number(/^take_(\d+)$/.exec(take.take_id)?.[1] ?? 0); }
function hasTakeColumn(slot: number) { return rows.value.some((row) => Boolean(row.takes[slot - 1])); }
function formatDuration(seconds: number) { return `${Math.floor(seconds / 60).toString().padStart(2, '0')}:${Math.floor(seconds % 60).toString().padStart(2, '0')}`; }
function formatCreatedAt(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? '' : date.toLocaleString(undefined, { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit' });
}
function formatHistoryTime(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? '' : date.toLocaleDateString(undefined, { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' });
}
const emit = defineEmits(['close', 'select-slide', 'toggle-recording', 'play', 'seek', 'make-golden', 'delete', 'autoplay', 'autoplay-take-column', 'export-master', 'export-video', 'exit-app', 'ingest-deck']);
function seek(event: Event, takeId: string, slideId: string) { emit('seek', takeId, Number((event.target as HTMLInputElement).value), slideId); }
function onHistoryDoubleClick(filePath: string) {
  activeTab.value = 'studio';
  emit('ingest-deck', filePath);
}

type StudioWindow = {
  startDragging: () => Promise<void>;
};
function currentWindow() {
  return (window as Window & { __TAURI__?: { window?: { getCurrentWindow: () => StudioWindow } } }).__TAURI__?.window?.getCurrentWindow();
}
function invokeWindowCommand<T>(command: string) {
  const invoke = (window as Window & { __TAURI__?: { core?: { invoke: <R>(command: string, args: object) => Promise<R> } } }).__TAURI__?.core?.invoke;
  return invoke ? invoke<T>(command, {}) : Promise.reject(new Error('Tauri window command is unavailable'));
}
function startWindowDrag(event: MouseEvent) {
  if (event.button !== 0 || (event.target instanceof Element && event.target.closest('button'))) return;
  void currentWindow()?.startDragging().catch(() => {});
}
function minimizeWindow() {
  void invokeWindowCommand<void>('minimize_studio_window').catch((error: unknown) => console.error('[SAISlideTake] Could not minimize Studio:', error));
}
async function toggleMaximize() {
  try {
    isMaximized.value = await invokeWindowCommand<boolean>('toggle_studio_window_maximize');
  } catch (error) {
    console.error('[SAISlideTake] Could not resize Studio:', error);
  }
}
watch(() => props.open, (open) => { if (open) isMaximized.value = false; });
</script>
