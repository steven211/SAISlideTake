import { ref } from 'vue';
import zhTW from './zh-TW.json';
import enUS from './en-US.json';
import zhCN from './zh-CN.json';
import jaJP from './ja-JP.json';
import koKR from './ko-KR.json';
import deDE from './de-DE.json';
import frFR from './fr-FR.json';
import esES from './es-ES.json';
import ptBR from './pt-BR.json';
import ruRU from './ru-RU.json';
import viVN from './vi-VN.json';

const dictionaries = {
  'zh-TW': zhTW,
  'en-US': enUS,
  'zh-CN': zhCN,
  'ja-JP': jaJP,
  'ko-KR': koKR,
  'de-DE': deDE,
  'fr-FR': frFR,
  'es-ES': esES,
  'pt-BR': ptBR,
  'ru-RU': ruRU,
  'vi-VN': viVN,
};
export type MessageKey = keyof typeof zhTW;
export type LocaleCode = keyof typeof dictionaries;

const locale = ref<LocaleCode>('zh-TW');

export function applyLocale() {
  const saved = localStorage.getItem('sai-slidetake-locale') as LocaleCode | null;
  const browser = navigator.language;
  const normalized = browser.toLowerCase();
  const language = normalized.split('-')[0];
  const matched = (Object.keys(dictionaries).find((key) => key.toLowerCase() === normalized)
    ?? Object.keys(dictionaries).find((key) => key.toLowerCase().startsWith(`${language}-`))) as LocaleCode | undefined;
  const chineseLocale = language === 'zh'
    ? (normalized.includes('cn') || normalized.includes('hans') ? 'zh-CN' : 'zh-TW')
    : undefined;
  setLocale(saved && saved in dictionaries ? saved : chineseLocale ?? matched ?? 'zh-TW');
}

export function setLocale(value: LocaleCode) {
  if (!(value in dictionaries)) return;
  locale.value = value;
  localStorage.setItem('sai-slidetake-locale', value);
  document.documentElement.lang = value;
}

export function useLocale() {
  const t = (key: MessageKey) => dictionaries[locale.value][key];
  return { locale, t };
}
