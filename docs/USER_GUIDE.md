# SAISlideTake User Guide

[English](USER_GUIDE.md) | [繁體中文](USER_GUIDE.zh-TW.md)

Welcome to **SAISlideTake (SAI Slide Audio Studio)**!  
SAISlideTake is an ultra-lightweight desktop presentation audio studio (DeskAPP) crafted for speakers, educators, course creators, and technical presenters. With our innovative **Slide Atomic Sharding** and **Immutable Append-Only Recording** paradigms, it permanently eliminates the frustration of having to re-record an entire presentation or spend hours cutting audio in complex video editors due to a single slip of the tongue.

---

## Table of Contents

1. [Core Design Principles](#1-core-design-principles)
2. [System Requirements & Quick Launch](#2-system-requirements--quick-launch)
3. [Supported Deck Formats & Loading](#3-supported-deck-formats--loading)
4. [Always-on-Top Floating Capsule](#4-always-on-top-floating-capsule)
5. [Seamless Auto-Chunking](#5-seamless-auto-chunking)
6. [Studio Matrix](#6-studio-matrix)
7. [Take Drawer & Golden Take Selection](#7-take-drawer--golden-take-selection)
8. [Internationalization & Deck Parser Configuration](#8-internationalization--deck-parser-configuration)
9. [Master Audio Export](#9-master-audio-export)
10. [Shortcut Cheatsheet](#10-shortcut-cheatsheet)

---

## 1. Core Design Principles

Traditional screen and slide recording carries high cognitive stress: a mistake on slide 20 forces you to either endure hours of timeline post-editing or start all over from slide 1.

SAISlideTake establishes a new recording paradigm:
* **Slide Atomic Sharding**: Audio is physically isolated per slide (`audio/<slide_id>/`). Modifying slide 5 never touches or invalidates slide 4 or slide 6.
* **No-Clobber Append-Only Recording**: Re-recording monotonically creates `Take 01`, `Take 02`, `Take 03`... Prior takes are never overwritten, allowing fearless rehearsal with different vocal styles.
* **Golden Take Pointer**: A lightweight `manifest.json` tracks the designated best version per slide with zero-latency instant switching and zero destruction.
* **Immersive Floating Overlay**: Dark glassmorphic floating capsule (`360x56` px) residing on the edge of your screen with bidirectional page sync latency < 16ms.

---

## 2. System Requirements & Quick Launch

### 2.1 Supported Platforms
* **macOS**: macOS 12.0 (Monterey) or higher (Native Apple Silicon M-series & Intel support).
* **Windows**: Windows 10 / 11 64-bit.

### 2.2 Portable Launch
SAISlideTake is a **Single Release standalone lightweight portable application**. The backend core and frontend UI are 100% statically embedded:

* **macOS**:
  1. Extract into any directory (or run directly from the release folder).
  2. Double-click `run-macos.sh` (which automatically strips macOS quarantine attributes) to launch `SAISlideTake.app`.
  3. Grant microphone permissions when prompted upon first launch.
* **Windows**:
  1. Double-click `run-windows.bat` or launch `SAISlideTake.exe` directly.

### 2.3 Optional Dependency: FFmpeg (Required Only for Export)
* Per-slide recording, playback, and Golden Take selection work **out-of-the-box without FFmpeg**.
* **FFmpeg** (100% free & open-source) is required on both **macOS** and **Windows** only when exporting Master Audio (WAV/MP3) or synthesizing MP4 teaching videos:
  * **macOS**: `brew install ffmpeg` (automatically detected at `/opt/homebrew/bin/ffmpeg`, `/usr/local/bin/ffmpeg`, or in `PATH`).
  * **Windows**: `winget install Gyan.FFmpeg` (or ensure `ffmpeg.exe` is in your system `PATH`).
  * **Custom Path**: You may also set the `SAIST_FFMPEG` environment variable to point directly to your `ffmpeg` executable.

---

## 3. Supported Deck Formats & Loading

SAISlideTake natively supports modern web presentation frameworks and traditional slide files:

1. **HTML5 Web Presentations**:
   * Out-of-the-box compatibility with **Reveal.js**, **Marp**, and **Slidev**.
   * Generic HTML decks with custom CSS selectors, heading splits (`<h1>` / `<h2>`), or `<hr>` tag separators.
2. **PowerPoint Presentations**:
   * Natively parses `.pptx` slide sequence, titles, and Speaker Notes without requiring Microsoft Office.

### 3.1 Loading a Deck
* **Drag & Drop**: Drag an HTML file or `.pptx` presentation onto the floating capsule.
* **Environment Variable**: Configure the default presentation directory via `SAIST_PRESENTATION_DIR`.

---

## 4. Always-on-Top Floating Capsule

The floating capsule features a dark glassmorphic styling (`backdrop-filter: blur(16px)`), staying subtly pinned to the edge of your presentation:

* **Page Index & Title**: Displays the current slide index (e.g., `[05/24]`) and slide title in real-time.
* **Recording Controls**:
  * **🎙️ Record**: Click the red button or press shortcut to begin recording the current slide.
  * **⏸️ Pause / Resume**: Pause at any time to gather thoughts.
  * **⏹️ Stop & Save**: Conclude the slide take and package it as the newest take for that slide.
* **Timer & Waveform**: Dynamic real-time elapsed timer accompanied by audio waveform activity feedback.
* **Expand Drawer**: Click the arrow icon on the right edge of the capsule to toggle the take history drawer.

---

## 5. Seamless Auto-Chunking

The most transformative feature for rehearsal workflow:

1. **Continuous Natural Rehearsal**: Press record on Slide 1 and speak freely without stopping after every slide.
2. **Zero-Friction Background Chunking**: Whenever the presenter advances the slide using a presentation clicker or keyboard (`→` / `Space` / `PageDown`):
   * Microphone capture **continues uninterrupted**.
   * In < 16ms, the previous slide's audio is finalized and saved as `audio/slide_01/take_01.wav`.
   * A fresh audio buffer is immediately opened for slide 2, saving as `audio/slide_02/take_01.wav`.
3. **Automatic Card Organization**: After a full run-through, every slide has an organized take card ready for review or individual re-recording.

---

## 6. Studio Matrix

Click the maximize button on the capsule (or press `Cmd+M` / `Ctrl+M`) to open the 1080x720 professional Studio Matrix:

* **Top Tab Navigation**:
  * **【🎙️ Recording Studio】**: The left sidebar shows the presentation slide tree (page indices, titles, recording state dots, speaker notes); the center pane displays the **Take 01 ～ Take 05** matrix grid; the right sidebar provides the **Deck History Sidebar**.
  * **【⚙️ Settings】**: Internationalization and custom deck parsing profile configuration.
* **Matrix Grid Controls**:
  * Click "▶ Preview" on any take card to audition audio.
  * Click "★ Set as Golden" to designate that take as the active playback version.
  * Delete specific unwanted takes with one click.
* **Deck History Sidebar**:
  * **Automated Deck Tracking**: Automatically persists up to 30 recently opened presentations (including file path, title, format, and slide count).
  * **Double-click Fast Ingest**: Double-click on any history card or click "📂 Import Deck" to instantly reload the deck and navigate to recording studio mode without re-browsing files.
  * **Flexible Management**: Supports individual item removal (✕) and one-click clear all history.

---

## 7. Take Drawer & Golden Take Selection

If you prefer to stay in the compact view while refining takes:
* Expand the lateral Take Cards Drawer right next to the floating capsule.
* Scroll through all historic takes for the active slide (Take 01, Take 02, Take 03...).
* Switch the Golden Take pointer immediately.
* Quick-audition audio to verify your re-recording before moving forward.

---

## 8. Internationalization & Deck Parser Configuration

Switch to the 【⚙️ Settings】 tab in the Studio Matrix:

### 8.1 Global 11 Language Switcher
* Real-time dropdown switching across 11 languages:
  * Traditional Chinese (zh-TW), English (en-US), Simplified Chinese (zh-CN)
  * Japanese (ja-JP), Korean (ko-KR), German (de-DE)
  * French (fr-FR), Spanish (es-ES), Portuguese (pt-BR)
  * Russian (ru-RU), Vietnamese (vi-VN)
* Instant UI text update without restarting the application.

### 8.2 Configurable Deck Parser Engine
* **Official Profiles**: Pre-configured profiles for Reveal.js, Marp, Slidev, generic H1/H2, generic class selectors, and PPTX.
* **JSON Rule Editor**: In-app JSON editor to inspect, customize, import, and export `.json` rule configurations.
* **▶ Test & Preview**: Test parser rules against the currently loaded presentation and get immediate feedback on detected total slides and slide titles.

---

## 9. Master Audio Export

Once golden takes are marked across all slides:
1. Open the Recording Studio tab and click **"Export Master Audio"** in the top right corner.
2. The engine concatenates each slide's Golden Take sequentially.
3. **Smart Silence Insertion**: For transition slides without spoken audio, the engine automatically injects a configurable duration of standard silence (default: 1.0s) to keep timing in sync with the visual deck.
4. Export high-quality lossless WAV or compressed formats ready for podcast publishing or video soundtrack integration.
   * *Note*: Requires **FFmpeg** installed on your system (see [Section 2.3](#23-optional-dependency-ffmpeg-required-only-for-export)).

---

## 10. Shortcut Cheatsheet

| Shortcut (macOS) | Shortcut (Windows) | Function |
| :--- | :--- | :--- |
| `Cmd + R` | `Ctrl + R` | Start / Stop recording on current slide |
| `Cmd + Space` | `Ctrl + Space` | Pause / Resume recording |
| `Cmd + M` | `Ctrl + M` | Toggle Studio Matrix modal window |
| `Cmd + D` | `Ctrl + D` | Toggle Take Cards drawer |
| `→` / `PageDown` | `→` / `PageDown` | Next slide (triggers automatic chunking during recording) |
| `←` / `PageUp` | `←` / `PageUp` | Previous slide (triggers automatic chunking during recording) |
| `Esc` | `Esc` | Close Studio Matrix or close active drawer |
