<template>
  <div class="settings-scroll">
    <section class="settings-card">
      <label class="settings-label" for="studio-language">{{ t('language') }}</label>
      <select id="studio-language" class="settings-select" :value="locale" @change="changeLanguage">
        <option v-for="option in languageOptions" :key="option.code" :value="option.code">{{ option.label }}</option>
      </select>
    </section>

    <section class="settings-card parser-card">
      <div class="settings-section-title">
        <div><h2>{{ t('parserSettings') }}</h2><p>{{ deck?.title || t('noDeckAvailable') }}</p></div>
        <label class="engine-field"><span>{{ t('parserEngine') }}</span>
          <select class="settings-select" :value="selectedEngine" @change="changeEngine">
            <option v-for="engine in engineOptions" :key="engine.id" :value="engine.id">{{ t(engine.key) }}</option>
          </select>
        </label>
      </div>
      <label class="settings-label" for="parser-profile-json">{{ t('profileJson') }}</label>
      <textarea id="parser-profile-json" class="profile-editor" spellcheck="false" :value="profileJson" @input="editProfile(($event.target as HTMLTextAreaElement).value)"></textarea>
      <input ref="fileInput" class="visually-hidden" type="file" accept="application/json,.json" @change="readProfile" />
      <div class="settings-actions">
        <button class="settings-button" @click="fileInput?.click()">{{ t('loadJson') }}</button>
        <button class="settings-button" @click="exportProfile">{{ t('exportJson') }}</button>
        <span class="settings-spacer"></span>
        <button class="settings-button primary" :disabled="!deck || isTesting" @click="testCurrentDeck">{{ isTesting ? t('testingParser') : t('testParser') }}</button>
        <button class="settings-button apply" :disabled="!deck || deck.format !== 'html' || selectedEngine === 'auto' || !profileValid || isApplying" @click="applyCurrentProfile">{{ isApplying ? t('applyingParser') : t('applyParser') }}</button>
      </div>
      <p v-if="selectedEngine !== 'auto' && selectedEngine !== 'pptx' && !profileValid" class="settings-feedback error">{{ t('parserInvalidProfile') }}</p>
      <p v-if="fileError || errorMessage" class="settings-feedback error" role="alert">{{ fileError || errorMessage }}</p>
      <p v-if="applyMessage" class="settings-feedback success" role="status">{{ t('parserApplied') }}</p>
    </section>

    <section v-if="testResult" class="settings-card parser-preview">
      <header><h2>{{ t('parserPreview') }}</h2><strong v-if="!testResult.error">{{ t('parserSlideCount').replace('{count}', String(testResult.total_slides)) }}</strong></header>
      <p v-if="testResult.error" class="preview-empty" role="alert">{{ testResult.error }}</p>
      <template v-else>
        <p v-if="!testResult.slides.length" class="preview-empty">{{ t('parserPreviewEmpty') }}</p>
        <ol v-else>
          <li v-for="slide in testResult.slides" :key="slide.slide_index">
            <span class="preview-index">{{ String(slide.slide_index + 1).padStart(2, '0') }}</span>
            <span class="preview-title">{{ slide.title }}</span>
            <span v-if="slide.active" class="preview-active">{{ t('parserCurrentSlide') }}</span>
            <small v-if="slide.notes" class="preview-notes">{{ slide.notes }}</small>
          </li>
        </ol>
      </template>
    </section>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import type { DeckInfo } from '../types';
import { useLocale, type LocaleCode } from '../locales';
import { useStudioSettings } from '../composables/useStudioSettings';

defineProps<{ deck: DeckInfo | null }>();
const { locale, t } = useLocale();
const {
  selectedEngine, profileJson, isTesting, isApplying, testResult, errorMessage, applyMessage,
  languageOptions, engineOptions, profileValid, selectEngine, editProfile, testCurrentDeck,
  applyCurrentProfile, exportProfile, importProfile, setLanguage,
} = useStudioSettings();
const fileInput = ref<HTMLInputElement | null>(null);
const fileError = ref('');
function changeLanguage(event: Event) { setLanguage((event.target as HTMLSelectElement).value as LocaleCode); }
function changeEngine(event: Event) { selectEngine((event.target as HTMLSelectElement).value); fileError.value = ''; }
async function readProfile(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  try { importProfile(await file.text()); fileError.value = ''; }
  catch { fileError.value = t('parserInvalidJson'); }
  input.value = '';
}
</script>
