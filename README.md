# SAISlideTake (Golden Take Presentation Studio)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Release: v0.1.0](https://img.shields.io/badge/Release-v0.1.0-blue.svg)](Cargo.toml)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-v2-blue.svg)](https://v2.tauri.app/)
[![Vue](https://img.shields.io/badge/Vue-3.4%2B-green.svg)](https://vuejs.org/)

[English](README.md) | [繁體中文](README.zh-TW.md)

> **"Every slide deserves its Golden Take. Rehearse per slide, replay as a masterpiece."**  
> 「每一頁，都是最好的選擇。」——單頁重錄不重來，自動連播，15 秒合成教學影片。

**SAISlideTake** is a high-performance desktop presentation voiceover studio and audio master synthesis platform designed for technical speakers, online course educators, and solo podcasters. Built with **Rust** (Tauri v2 + CPAL native PCM audio pipeline) and a modern **Vue 3** frontend, it completely revolutionizes the painful "40-minute linear recording, re-speaking from scratch upon a mistake, or spending hours in video editors" workflow—transplanting the professional recording studio philosophy of **"Multi-Take Comping"** directly into slide presentations.

---

## About SAISoft

**SAISoft** (*Steven's AI Soft*) is an independent software development studio focused on AI-driven software engineering.

> **"I have established an engineering system that enables AI to continuously deliver production-ready software under controlled architecture, clear specifications, and strict quality standards."**

This project demonstrates three distinct layers of capability:
* **Layer 1: Product Capability**  
  Pioneering a "Digital Audio Workstation for Slides (DAW for Slides)" featuring an always-on-top floating capsule, seamless auto-chunking during live rehearsal, visual Take Cards for Golden Take selection, and 15-second offline video synthesis into 4K instructional MP4s.
* **Layer 2: Engineering Capability**  
  Designing an ultra-lightweight architecture (~10MB binary, minimal memory footprint), enforcing a strict "Vue single-file < 10KB physical redline (Trinity Trigger)", thread-safe Rust `Arc` DI container, low-latency CPAL PCM audio capture, and robust cross-platform window levels across macOS (AppKit / AppleScript) and Windows (Win32 / COM).
* **Layer 3: Development Paradigm Capability**  
  Establishing a disciplined AI engineering system that enables autonomous coding agents to iteratively build, refactor, and deliver production-grade software under strict human-defined specifications and quality gates—eliminating random trial-and-error and the need to start over each time.

Our software is crafted through **100% AI-assisted coding**, directed and governed by **human-designed architecture, detailed requirements engineering, rigorous code reviews, and quality assurance**.

### Project Profile

| Attribute | Details |
| :--- | :--- |
| **Product** | **SAISlideTake (Golden Take Presentation Studio)** |
| **Developer** | **SAISoft (Steven's AI Soft)** |
| **Project Nature** | **Independent Experimental / Lab Project** (自我研發 Lab 型專案) |
| **Pricing** | **100% Completely Free & Open Source** (完全免費) |
| **Development Model** | **100% AI-Assisted Coding** |
| **Architecture** | **Human-Directed Modular Monolith & Local-First Architecture** |
| **License** | **MIT License** |
| **Tech Stack** | **Rust (Tauri v2 + CPAL + Axum) + Vue 3 (TypeScript + CSS Glassmorphism)** |
| **Slide Support** | **HTML5 (Reveal.js, Marp, Slidev, Generic DOM) + PowerPoint (.pptx)** |
| **Locales** | **Global 11 Matrix** (zh-TW, en-US, zh-CN, ja-JP, ko-KR, de-DE, fr-FR, es-ES, pt-BR, ru-RU, vi-VN) |

---

## Key Features

### 1. Always-on-Top Floating Capsule Toolbar
* **Minimalist Immersive Overlay**: Deep dark translucent glassmorphism (`360x56` px, `backdrop-filter: blur(16px)`), residing gently at the edge of the screen without obstructing presentation content.
* **Bidirectional Low-Latency Sync**: Slide transition events sync to the floating toolbar via local WebSocket / WebAPI in **< 16ms**, dynamically updating page badges `[05/24]`, slide titles, and timers.
* **System Full-Screen Penetration**: Leverages `NSWindowCollectionBehaviorFullScreenAuxiliary` across all Spaces on macOS; re-asserts topmost status intelligently on window blur in Windows.

### 2. Slide-Level Atomic Sharding & No-Clobber Storage
* **Physical Slide Isolation**: Audio is sharded and stored independently under atomic slide directories (`audio/<slide_id>/take_N.wav`).
* **Immutable Version History**: New takes are strictly appended monotonically (Take 01, Take 02, Take 03...), never overwriting historical takes. Speakers always keep their safety nets and stay completely confident.

### 3. Continuous Rehearsal with Seamless Auto-Chunking
* **Uninterrupted Speaker Flow**: Press Record on Slide 1 and speak through all 30 slides just like delivering a real talk on stage.
* **Silent Background Slicing**: Every time the speaker presses the presenter remote or arrow key (`→` / `Space`), the microphone stream continues rolling seamlessly while the backend seals the previous slide's WAV and opens a fresh buffer for the next slide. Experience live continuous rehearsal while automatically obtaining a full suite of per-slide Take cards!

### 4. Advanced Studio Matrix & Top Tab Navigation
* **Global Command Grid**: One click expands the floating capsule into an opaque, professional 1080x720 studio workspace with draggable cross-platform custom title bars.
* **Top Tab Navigation**:
  * **【🎙️ Recording Studio】**: Left column lists all slides (page numbers, titles, green/gray recording status indicators, speaker notes); right matrix displays **Take 01 ～ Take 05** cards with waveform preview, instant **★ Golden Take** designation, and deletion.
  * **【⚙️ Settings】**: Integrated Global 11 language switching and configurable deck parser profiles.

### 5. Configurable JSON Deck Parser Engine
* **Out-of-the-Box Ingestion**: Native auto-detection for Reveal.js (`<section>`), Marp (`.marp-slide`), Slidev, generic semantic H1/H2 headings, and PowerPoint (.pptx) OpenXML static unpacking.
* **Custom JSON Profiles**: Multi-line code editor allowing users to define custom `slide_selector`, `title_selector`, and `notes_selector`, with JSON file import and export capabilities.
* **Instant Test & Preview**: "▶ Test Current Deck Parser" button provides immediate feedback within 0.1s, listing detected slide counts and titles to eliminate guesswork.

### 6. Zero-Loss Master Audio Export & 15-Second 4K Video Synthesis
* **One-Click Master Audio**: Reads `slidetake-manifest.json`, smooths slide boundaries with subtle cross-fades, and exports a unified high-fidelity audio master (WAV / MP3).
* **Instant Offline Video Synthesis**: Extracts high-resolution slide frames via headless browser, aligns timing strictly with each slide's Golden Take duration, and encodes directly via an offline FFmpeg pipeline into H.264/AAC 4K MP4. A 1-hour lecture compiles in just 15~20 seconds—say goodbye to tedious video editors!

---

## Architecture Overview

SAISlideTake adheres to the **Modular Monolith** pattern and **Local-First** design principles:

```text
┌────────────────────────────────────────────────────────────────────────┐
│                     Presentation & Client Layers                       │
│  ┌──────────────────────────────────┐  ┌────────────────────────────┐  │
│  │     Tauri v2 Desktop Shell       │  │   Vue 3 Web Presentation   │  │
│  │   (Floating Capsule / Studio)    │  │ (Reveal.js / Marp / Slidev)│  │
│  └──────────────────┬───────────────┘  └─────────────┬──────────────┘  │
└─────────────────────┼────────────────────────────────┼─────────────────┘
                      │ (Local REST / WebSocket :3188) │
┌─────────────────────▼────────────────────────────────▼─────────────────┐
│              saist-core (Rust Modular Monolith Core Control)           │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │         Axum HTTP / WebSocket Event Hub & Deck Ingestion         │  │
│  └─────────────────┬───────────────────────────────┬────────────────┘  │
│                    │                               │                   │
│  ┌─────────────────▼──────────────┐  ┌─────────────▼────────────────┐  │
│  │ CPAL Native PCM Audio Stream   │  │ DeckParser (HTML / OpenXML)  │  │
│  └─────────────────┬──────────────┘  └─────────────┬────────────────┘  │
│                    │                               │                   │
│  ┌─────────────────▼──────────────┐  ┌─────────────▼────────────────┐  │
│  │ Audio Storage & Atomic Sharding│  │ FFmpeg Concat Video Engine   │  │
│  └─────────────────┬──────────────┘  └─────────────┬────────────────┘  │
└────────────────────┼───────────────────────────────┼───────────────────┘
                     │                               │
┌────────────────────▼───────────────────────────────▼───────────────────┐
│                   Local Presentation Root Directory                    │
│  ├── presentation.html (or .pptx)                                      │
│  └── audio/                                                            │
│      ├── slidetake-manifest.json (Single Source of Truth)              │
│      ├── slide_01_Introduction/take_01.wav, take_02.wav               │
│      └── slide_02_Architecture/take_01.wav                            │
└────────────────────────────────────────────────────────────────────────┘
```

---

## Project Structure

```text
SAISlideTake/
├── 9.output/                     # Unified compilation & release output directory
├── docs/                         # User manuals
│   ├── USER_GUIDE.md             # User Guide (English)
│   └── USER_GUIDE.zh-TW.md       # User Guide (Traditional Chinese)
├── proto/                        # Protobuf / gRPC service contracts (slide_take.proto)
├── src/
│   ├── saist-core/               # Rust local core (Axum, CPAL, DeckParser, AudioStorage)
│   ├── saist-portal/             # Tauri desktop shell (window penetration, drag-and-drop)
│   └── saist-web/                # Vue 3 frontend (capsule, studio matrix, 11 locales)
│       └── public/
│           └── sai-slidetake-bridge.js # Presentation-side bidirectional SDK
├── scripts/                      # Cross-platform debug & build scripts (macos/, win/)
└── Cargo.toml                    # Rust workspace configuration
```

---

## Quick Start

### Prerequisites
- **Rust**: 1.85+ (`cargo`, `rustc`)
- **Node.js**: 20+ (`npm`)
- **FFmpeg**: Free & open-source; required on both macOS (`brew install ffmpeg`) and Windows (`winget install Gyan.FFmpeg` or `SAIST_FFMPEG` env var) for audio master merging and video synthesis (not needed for daily slide recording)

### Running in Development

```bash
# 1. Install frontend dependencies
cd src/saist-web && npm install && cd ../..

# 2. Launch with cross-platform HMR debug script
# macOS:
./scripts/macos/SAISlideTake_DEBUG.sh

# Windows:
.\scripts\win\SAISlideTake_DEBUG.bat
```

### Verification & Quality Gates

```bash
# Verify Rust backend unit tests and compilation
cargo test --workspace
cargo check --workspace

# Verify frontend TypeScript types and production build
cd src/saist-web
npm run typecheck
npm run build
```

---

## License

This project is open-sourced under the **[MIT License](LICENSE)**. You are free to modify, distribute, and use it commercially. See the LICENSE file for details.
