export interface TakeCard {
  take_id: string;
  slide_id: string;
  filename: string;
  relative_path: string;
  duration_sec: number;
  created_at: string;
  note: string;
  is_golden: boolean;
}

export interface TakeGroup {
  slide_id: string;
  slide_title: string;
  golden_take_id: string | null;
  takes: TakeCard[];
}

export interface DeckSlide {
  slide_id: string;
  slide_index: number;
  title: string;
  notes?: string;
}

export interface DeckInfo {
  deck_id: string;
  title: string;
  source_path: string;
  format: string;
  bridge_path?: string | null;
  slides: DeckSlide[];
}

export interface PresentationManifest {
  schema_version: string;
  presentation_id: string;
  transition_delay_ms: number;
  slides: Record<string, TakeGroup>;
}

export interface SlideChangedEvent {
  type: 'slide_changed';
  payload: { slide_id: string; slide_index: number; total_slides: number; slide_title: string };
}

export interface BridgeEvent {
  type: string;
  payload?: unknown;
}
