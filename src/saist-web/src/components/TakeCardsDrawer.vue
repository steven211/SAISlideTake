<template>
  <aside v-if="open" class="take-drawer" :aria-label="t('takes')">
    <header class="drawer-heading">
      <div><span>{{ t('takes') }}</span><small>{{ slideTitle }}</small></div>
      <button class="close-button" :aria-label="t('close')" @click="$emit('close')">×</button>
    </header>
    <div v-if="!takes.length" class="empty-state">{{ t('emptyTakes') }}</div>
    <div v-else class="take-list">
      <article v-for="take in takes" :key="take.take_id" class="take-card" :class="{ golden: take.take_id === goldenTakeId }">
        <div class="take-card-heading">
          <strong>{{ t('take') }} {{ takeNumber(take.take_id) }}</strong>
          <span v-if="take.take_id === goldenTakeId" class="golden-badge">{{ t('golden') }}</span>
        </div>
        <div class="take-meta"><time>{{ t('recordedAt') }} {{ formatCreatedAt(take.created_at) }}</time><span>{{ t('duration') }} {{ formatDuration(take.duration_sec) }}</span></div>
        <div class="take-progress">
          <input type="range" min="0" :max="takeProgressMax(take)" :value="takeProgress(take)" :disabled="!isCurrentTakePlaying(take)" :aria-label="t('play')" @input="seek(take.take_id, $event, take.slide_id)" />
          <small>{{ formatDuration(takeProgress(take)) }} / {{ formatDuration(takeProgressMax(take)) }}</small>
        </div>
        <div class="take-actions">
          <button class="play-button" @click="$emit('play', take)">{{ isCurrentTakePlaying(take) && !isPreviewPaused ? '❚❚' : t('play') }}</button>
          <button v-if="take.take_id !== goldenTakeId" class="golden-button" @click="$emit('make-golden', take.take_id)">{{ t('makeGolden') }}</button>
          <button class="delete-button" :aria-label="t('delete')" @click="$emit('delete', take.take_id)">🗑</button>
        </div>
        <label class="note-label">{{ t('note') }}
          <input v-model="notes[take.take_id]" :placeholder="t('enterNote')" @change="saveNote(take.take_id, $event)" />
        </label>
      </article>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { reactive, watch } from 'vue';
import type { TakeCard } from '../types';
import { useLocale } from '../locales';

const props = defineProps<{
  open: boolean;
  slideTitle: string;
  takes: TakeCard[];
  goldenTakeId: string | null;
  playingTakeId: string | null;
  playingSlideId?: string | null;
  isPreviewPaused: boolean;
  playbackPosition: number;
  playbackDuration: number;
}>();

const emit = defineEmits(['close', 'play', 'seek', 'make-golden', 'delete', 'save-note']);
const { t } = useLocale();
const notes = reactive<Record<string, string>>({});

watch(() => props.takes, (takes) => {
  for (const take of takes) notes[take.take_id] = take.note;
}, { immediate: true, deep: true });

function isCurrentTakePlaying(take: TakeCard) {
  return props.playingTakeId === take.take_id && (!props.playingSlideId || props.playingSlideId === take.slide_id);
}
function takeNumber(takeId: string) { return takeId.replace(/^take_/, ''); }
function formatDuration(seconds: number) { return `${Math.floor(seconds / 60)}:${Math.floor(seconds % 60).toString().padStart(2, '0')}`; }
function formatCreatedAt(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? '' : date.toLocaleString(undefined, { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit' });
}
function takeProgress(take: TakeCard) { return isCurrentTakePlaying(take) ? props.playbackPosition : 0; }
function takeProgressMax(take: TakeCard) {
  return isCurrentTakePlaying(take) && props.playbackDuration > 0 ? props.playbackDuration : Math.max(take.duration_sec, 0.1);
}
function seek(takeId: string, event: Event, slideId?: string) { emit('seek', takeId, Number((event.target as HTMLInputElement).value), slideId); }
function saveNote(takeId: string, event: Event) {
  const input = event.target as HTMLInputElement;
  if (input) emit('save-note', takeId, input.value);
}
</script>
