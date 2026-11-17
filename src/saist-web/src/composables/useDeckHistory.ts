import { ref } from 'vue';
import type { DeckInfo } from '../types';

export interface DeckHistoryItem {
  filePath: string;
  title: string;
  format: string;
  slidesCount: number;
  lastOpenedAt: string;
}

const STORAGE_KEY = 'sai-slidetake-deck-history';
const MAX_HISTORY = 30;

function loadHistory(): DeckHistoryItem[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function saveHistory(list: DeckHistoryItem[]) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(list));
  } catch {
    // 忽略儲存例外
  }
}

const historyList = ref<DeckHistoryItem[]>(loadHistory());

export function useDeckHistory() {
  function addDeckToHistory(deck: DeckInfo | null) {
    if (!deck || !deck.source_path || !deck.source_path.trim()) return;
    const path = deck.source_path.trim();
    const existingIndex = historyList.value.findIndex((item) => item.filePath === path);
    const item: DeckHistoryItem = {
      filePath: path,
      title: deck.title || path.split('/').pop()?.split('\\').pop() || 'Untitled Presentation',
      format: deck.format || 'unknown',
      slidesCount: deck.slides?.length || 0,
      lastOpenedAt: new Date().toISOString(),
    };
    const nextList = [...historyList.value];
    if (existingIndex >= 0) {
      nextList.splice(existingIndex, 1);
    }
    nextList.unshift(item);
    if (nextList.length > MAX_HISTORY) {
      nextList.length = MAX_HISTORY;
    }
    historyList.value = nextList;
    saveHistory(nextList);
  }

  function removeHistoryItem(filePath: string) {
    historyList.value = historyList.value.filter((item) => item.filePath !== filePath);
    saveHistory(historyList.value);
  }

  function clearHistory() {
    historyList.value = [];
    saveHistory([]);
  }

  return {
    historyList,
    addDeckToHistory,
    removeHistoryItem,
    clearHistory,
  };
}
