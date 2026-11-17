<template>
  <main class="saist-shell" :class="{ 'drawer-open': drawerOpen, 'studio-active': studioOpen }">
    <FloatingCapsuleBar
      :current-slide-index="currentSlideIndex"
      :total-slides="totalSlides"
      :slide-title="slideTitle"
      :is-recording="isRecording"
      :recording-seconds="recordingSeconds"
      :waveform="waveform"
      :take-count="takes.length"
      :connection="connection"
      :is-dragging-over="isDraggingOver"
      @toggle-recording="toggleRecording"
      @replay="replayGolden"
      @toggle-drawer="toggleDrawer"
      @open-studio="toggleStudio(true)"
    />
    <TakeCardsDrawer
      :open="drawerOpen"
      :slide-title="slideTitle"
      :takes="takes"
      :golden-take-id="goldenTakeId"
      :playing-take-id="playingTakeId"
      :playing-slide-id="playingSlideId"
      :is-preview-paused="isPreviewPaused"
      :playback-position="playbackPosition"
      :playback-duration="playbackDuration"
      @close="toggleDrawer(false)"
      @play="playTake"
      @seek="seekTake"
      @make-golden="setGolden"
      @delete="requestDeleteTake"
      @save-note="saveNote"
    />
    <StudioMatrixModal
      :open="studioOpen"
      :deck="deck"
      :manifest="manifest?.slides || null"
      :selected-slide-id="selectedSlideId"
      :playing-take-id="playingTakeId"
      :playing-slide-id="playingSlideId"
      :is-preview-paused="isPreviewPaused"
      :playback-position="playbackPosition"
      :playback-duration="playbackDuration"
      :is-recording="isRecording"
      :is-autoplaying="isAutoplaying"
      :video-exporting="videoExporting"
      :video-export-progress="videoExportProgress"
      @close="toggleStudio(false)"
      @select-slide="selectStudioSlide"
      @toggle-recording="toggleRecording"
      @play="playTake"
      @seek="seekTake"
      @make-golden="setGolden"
      @delete="requestDeleteTake"
      @autoplay="toggleAutoplay"
      @autoplay-take-column="autoplayTakeColumn"
      @export-master="exportMasterAudio"
      @export-video="exportVideo"
      @exit-app="exitApplication"
      @ingest-deck="ingestDeck"
    />
    <div v-if="pendingDeleteTake" class="confirm-backdrop" @click.self="cancelDeleteTake">
      <section class="delete-confirm" role="alertdialog" aria-modal="true">
        <p>{{ t('deleteConfirm') }}</p>
        <footer><button @click="cancelDeleteTake">{{ t('cancel') }}</button><button class="confirm-danger" @click="confirmDeleteTake">{{ t('confirmDelete') }}</button></footer>
      </section>
    </div>
    <p v-if="errorMessage" class="toast" role="status">{{ errorMessage }}</p>
  </main>
</template>

<script setup lang="ts">
import FloatingCapsuleBar from './components/FloatingCapsuleBar.vue';
import TakeCardsDrawer from './components/TakeCardsDrawer.vue';
import StudioMatrixModal from './components/StudioMatrixModal.vue';
import { useSlideTakeEngine } from './composables/useSlideTakeEngine';
import { useLocale } from './locales';
const { t } = useLocale();

const {
  currentSlideIndex, totalSlides, slideTitle, isRecording, recordingSeconds,
  waveform, takes, goldenTakeId, playingTakeId, playingSlideId, isPreviewPaused,
  playbackPosition, playbackDuration, connection, isAutoplaying,
  drawerOpen, errorMessage, toggleRecording, replayGolden, toggleAutoplay, autoplayTakeColumn,
  toggleDrawer, playTake, seekTake, setGolden, requestDeleteTake, confirmDeleteTake, cancelDeleteTake, saveNote,
  deck, manifest, selectedSlideId, studioOpen, isDraggingOver,
  toggleStudio, selectStudioSlide, exportMasterAudio,
  videoExporting, videoExportProgress, exportVideo, exitApplication,
  ingestDeck,
  pendingDeleteTake,
} = useSlideTakeEngine();
</script>
