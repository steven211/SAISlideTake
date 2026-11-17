<template>
  <header class="capsule" :class="{ 'drag-active': isDraggingOver }" @mousedown="startDragging">
    <div class="slide-label" :title="slideTitle || t('unknownSlide')">
      <small>{{ slideBadge }}</small><strong>{{ slideTitle || t('unknownSlide') }}</strong>
    </div>
    <span v-if="isRecording" class="rec-timer"><i />{{ formatTime(recordingSeconds) }}</span>
    <div v-if="isRecording" class="waveform" :aria-label="t('waveform')">
      <b v-for="(level, index) in waveform" :key="index" :style="{ height: `${Math.max(3, level * 23)}px` }" />
    </div>
    <button class="icon-button record-button" :class="{ recording: isRecording }" :title="isRecording ? t('stop') : t('record')" :aria-label="isRecording ? t('stop') : t('record')" @click="$emit('toggle-recording')">
      {{ isRecording ? '■' : '●' }}
    </button>
    <button class="icon-button" :title="t('replayGolden')" :aria-label="t('replayGolden')" @click="$emit('replay')">▶</button>
    <button class="icon-button drawer-button" :title="t('takes')" :aria-label="t('takes')" @click="$emit('toggle-drawer')">▤<sup>{{ takeCount }}</sup></button>
    <button class="icon-button" :title="t('studio')" :aria-label="t('studio')" @click="$emit('open-studio')">⛶</button>
    <span class="connection-dot" :class="connection" :title="connectionLabel" />
    <span v-if="isDraggingOver" class="drop-hint">{{ t('dragHint') }}</span>
  </header>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useLocale } from '../locales';

const props = defineProps<{
  currentSlideIndex: number;
  totalSlides: number;
  slideTitle: string;
  isRecording: boolean;
  recordingSeconds: number;
  waveform: number[];
  takeCount: number;
  connection: 'connecting' | 'online' | 'offline';
  isDraggingOver: boolean;
}>();

defineEmits(['toggle-recording', 'replay', 'toggle-drawer', 'open-studio']);
const { t } = useLocale();
const slideBadge = computed(() => `[${String(props.currentSlideIndex + 1).padStart(2, '0')}/${String(props.totalSlides || 0).padStart(2, '0')}]`);
const connectionLabel = computed(() => t(`connection${props.connection[0].toUpperCase()}${props.connection.slice(1)}` as 'connectionConnecting' | 'connectionOnline' | 'connectionOffline'));

type TauriWindowApi = {
  window?: { getCurrentWindow: () => { startDragging: () => Promise<void> } };
};

function startDragging(event: MouseEvent) {
  if (event.button !== 0) return;
  const target = event.target;
  if (target instanceof Element && target.closest('button, input, textarea, select, [data-no-window-drag]')) return;
  const tauri = (window as Window & { __TAURI__?: TauriWindowApi }).__TAURI__;
  void tauri?.window?.getCurrentWindow().startDragging().catch(() => {});
}

function formatTime(seconds: number) {
  const minutes = Math.floor(seconds / 60).toString().padStart(2, '0');
  return `${minutes}:${Math.floor(seconds % 60).toString().padStart(2, '0')}`;
}
</script>
