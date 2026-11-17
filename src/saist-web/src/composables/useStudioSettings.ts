import { computed, ref } from 'vue';
import { setLocale, useLocale, type LocaleCode, type MessageKey } from '../locales';

const API = 'http://127.0.0.1:3188';
const { t } = useLocale();
export interface DeckParserProfile {
  engine_name: string;
  slide_selector: string;
  active_class: string;
  title_selector: string;
  notes_selector: string;
  fallback_strategy: 'heading_h1' | 'heading_h2' | 'hr_split' | 'manual';
}
export interface ParserTestResult {
  total_slides: number;
  slides: Array<{ slide_index: number; title: string; notes?: string; active: boolean }>;
  error?: string | null;
}

const genericProfile: DeckParserProfile = {
  engine_name: 'Generic Class', slide_selector: '.slide, .chapter', active_class: 'active, present, current',
  title_selector: '[data-title], h1, h2, .slide-title', notes_selector: '', fallback_strategy: 'manual',
};
const presets: Record<string, DeckParserProfile | null> = {
  auto: null,
  reveal: { ...genericProfile, engine_name: 'Reveal.js', slide_selector: '.slides > section', active_class: 'present, active', title_selector: '[data-title], h1, h2, .slide-title' },
  marp: { ...genericProfile, engine_name: 'Marp', slide_selector: '.marp-slide, section[data-marpit-pagination], section[data-marpit-pagination-total]' },
  slidev: { ...genericProfile, engine_name: 'Slidev', slide_selector: '.slidev-page, [data-slidev-page]' },
  headingH1: { ...genericProfile, engine_name: 'Generic Heading H1', slide_selector: 'h1', title_selector: 'h1' },
  headingH2: { ...genericProfile, engine_name: 'Generic Heading H2', slide_selector: 'h2', title_selector: 'h2' },
  class: genericProfile,
  pptx: null,
  custom: genericProfile,
};

function initialEngine() {
  return localStorage.getItem('sai-slidetake-parser-engine') || 'auto';
}
function initialJson() {
  const saved = localStorage.getItem('sai-slidetake-parser-profile');
  if (saved) return saved;
  return JSON.stringify(genericProfile, null, 2);
}

const selectedEngine = ref(initialEngine());
const profileJson = ref(initialJson());
const isTesting = ref(false);
const isApplying = ref(false);
const testResult = ref<ParserTestResult | null>(null);
const errorMessage = ref('');
const applyMessage = ref('');

function saveSettings() {
  localStorage.setItem('sai-slidetake-parser-engine', selectedEngine.value);
  localStorage.setItem('sai-slidetake-parser-profile', profileJson.value);
}
function getProfile(): DeckParserProfile | null {
  if (selectedEngine.value === 'auto' || selectedEngine.value === 'pptx') return null;
  const profile = JSON.parse(profileJson.value) as Partial<DeckParserProfile> | null;
  if (!profile || Array.isArray(profile) || typeof profile.engine_name !== 'string' || !profile.engine_name.trim()
    || typeof profile.slide_selector !== 'string' || !profile.slide_selector.trim()) {
    throw new Error(t('parserInvalidProfile'));
  }
  const fallback = profile.fallback_strategy || 'manual';
  if (!['heading_h1', 'heading_h2', 'hr_split', 'manual'].includes(fallback)) {
    throw new Error(t('parserInvalidProfile'));
  }
  return {
    engine_name: profile.engine_name,
    slide_selector: profile.slide_selector,
    active_class: profile.active_class || '',
    title_selector: profile.title_selector || '',
    notes_selector: profile.notes_selector || '',
    fallback_strategy: fallback,
  };
}
function selectEngine(engine: string) {
  selectedEngine.value = engine;
  if (presets[engine]) profileJson.value = JSON.stringify(presets[engine], null, 2);
  saveSettings();
  clearFeedback();
}
function editProfile(value: string) {
  profileJson.value = value;
  selectedEngine.value = 'custom';
  saveSettings();
  clearFeedback();
}
function clearFeedback() {
  errorMessage.value = '';
  applyMessage.value = '';
  testResult.value = null;
}
function parserProfileForIngest() {
  return getProfile();
}
async function responseJson<T>(response: Response): Promise<T> {
  const payload = await response.json().catch(() => ({})) as T & { error?: string };
  if (!response.ok) throw new Error(payload.error || `HTTP ${response.status}`);
  return payload;
}
async function testCurrentDeck() {
  isTesting.value = true;
  clearFeedback();
  try {
    testResult.value = await responseJson<ParserTestResult>(await fetch(`${API}/api/deck/test-parser`, {
      method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(getProfile()),
    }));
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  } finally { isTesting.value = false; }
}
async function applyCurrentProfile() {
  isApplying.value = true;
  clearFeedback();
  try {
    const profile = getProfile();
    if (!profile) throw new Error('Select an HTML parser profile before applying it');
    await responseJson(await fetch(`${API}/api/deck/apply-parser`, {
      method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(profile),
    }));
    applyMessage.value = 'Parser profile applied';
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  } finally { isApplying.value = false; }
}
function exportProfile() {
  const blob = new Blob([profileJson.value], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = 'saislidtake-parser-profile.json';
  link.click();
  URL.revokeObjectURL(url);
}
function importProfile(value: string) {
  editProfile(JSON.stringify(JSON.parse(value), null, 2));
}

export const languageOptions: Array<{ code: LocaleCode; label: string }> = [
  { code: 'zh-TW', label: '繁體中文' }, { code: 'en-US', label: 'English (US)' }, { code: 'zh-CN', label: '简体中文' },
  { code: 'ja-JP', label: '日本語' }, { code: 'ko-KR', label: '한국어' }, { code: 'de-DE', label: 'Deutsch' },
  { code: 'fr-FR', label: 'Français' }, { code: 'es-ES', label: 'Español' }, { code: 'pt-BR', label: 'Português (Brasil)' },
  { code: 'ru-RU', label: 'Русский' }, { code: 'vi-VN', label: 'Tiếng Việt' },
];
export const engineOptions: Array<{ id: string; key: MessageKey }> = [
  { id: 'auto', key: 'engineAuto' }, { id: 'reveal', key: 'engineReveal' }, { id: 'marp', key: 'engineMarp' },
  { id: 'slidev', key: 'engineSlidev' }, { id: 'headingH1', key: 'engineHeadingH1' },
  { id: 'headingH2', key: 'engineHeadingH2' }, { id: 'class', key: 'engineGenericClass' },
  { id: 'pptx', key: 'enginePowerPoint' }, { id: 'custom', key: 'engineCustom' },
];

export function useStudioSettings() {
  return {
    selectedEngine, profileJson, isTesting, isApplying, testResult, errorMessage, applyMessage,
    languageOptions, engineOptions,
    profileValid: computed(() => { try { getProfile(); return true; } catch { return false; } }),
    selectEngine, editProfile, testCurrentDeck, applyCurrentProfile, importProfile, exportProfile,
    parserProfileForIngest, setLanguage: setLocale,
  };
}
