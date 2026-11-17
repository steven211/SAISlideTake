# SAISlideTake (SAI 簡報分鏡錄音室)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Release: v0.1.0](https://img.shields.io/badge/Release-v0.1.0-blue.svg)](Cargo.toml)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-v2-blue.svg)](https://v2.tauri.app/)
[![Vue](https://img.shields.io/badge/Vue-3.4%2B-green.svg)](https://vuejs.org/)

[English](README.md) | [繁體中文](README.zh-TW.md)

> **"Every slide deserves its Golden Take. Rehearse per slide, replay as a masterpiece."**  
> 「每一頁，都是最好的選擇。」——單頁重錄不重來，自動連播，15 秒合成教學影片。

**SAISlideTake** 是一套專為技術講者、線上課程教師與知識型播客（Podcaster）打造的桌面級簡報分鏡錄音工作台與語音母帶合成中台。採用 Rust（Tauri v2 + CPAL 原生 PCM 音訊管線）與 Vue 3 現代化前端架構，徹底顛覆傳統「40 分鐘線性錄影、吃螺絲重來或耗費數小時剪輯」的痛苦流程，將專業錄音室的「多版本試錄（Multi-Take Comping）」哲學降維移植至投影片演練場景。

---

## 關於 SAISoft (About SAISoft)

**SAISoft** (*Steven's AI Soft*) 是一個專注於 AI 驅動軟體工程（AI-driven software engineering）的獨立軟體開發工作室。

> **「我建立了一套工程系統，能讓 AI 在可控的架構、規格和品質標準下，持續交付正式可用的軟體。」**

本專案展現了三個層次的核心能力：
* **第一層：產品能力 (Product Capability)**  
  開創「簡報界的數位音樂工作站（DAW for Slides）」，實現置頂浮動膠囊、連續演練換頁自動切檔、版本卡片挑選黃金錄音（Golden Take）、以及秒級影音離線合流輸出 4K 教學影片。
* **第二層：工程能力 (Engineering Capability)**  
  設計極致羽量級架構（~10MB 記憶體佔用極低），堅持「Vue 單檔 < 10KB 物理紅線（三位一體拆分）」、Rust 後端線程安全 `Arc` DI 容器、CPAL 低延遲 PCM 採集、與跨 macOS（AppKit / AppleScript）/ Windows（Win32 / COM）的強健原生視窗與放映控制。
* **第三層：開發模式能力 (Development Paradigm Capability)**  
  建立可控的 AI 協作工程體系，讓 AI 在明確規格定義與嚴密品質防護網下，具備可重複、可疊代、持續完成多個專案的工程化交付能力，而不是每次重新開始。

我們的軟體完全透過 **100% AI 輔助編程（AI-assisted coding）** 開發，並由**人類主導架構設計、需求規格定義、嚴謹的代碼審查（Code Review）與品質保證（Quality Assurance）**。

### 專案概要 (Project Profile)

| 屬性項目 | 規格與說明 |
| :--- | :--- |
| **產品名稱** | **SAISlideTake (SAI 簡報分鏡錄音室)** |
| **開發團隊** | **SAISoft (Steven's AI Soft)** |
| **專案性質** | **獨立研發實驗型專案（Lab Project）**，開源工具型軟體 |
| **授權與費用** | **100% 完全免費與開源（Completely Free & Open Source）** |
| **開發模式** | **100% AI 輔助代碼撰寫（AI Coding）** |
| **架構主導** | **人類架構師主導（Human-directed Architecture & QA）** |
| **授權條款** | **MIT License** |
| **技術棧架構** | **Rust (Tauri v2 + CPAL + Axum) + Vue 3 (TypeScript + CSS Glassmorphism)** |
| **簡報支援度** | **HTML5 (Reveal.js, Marp, Slidev, 通用語意) + PowerPoint (.pptx)** |
| **語言支援** | **Global 11 國際化矩陣** (zh-TW, en-US, zh-CN, ja-JP, ko-KR, de-DE, fr-FR, es-ES, pt-BR, ru-RU, vi-VN) |

---

## 核心功能特色 (Key Features)

### 1. 置頂半透明膠囊浮動列 (Always-on-Top Floating Capsule)
* **極簡沉浸懸浮**：深色半透明毛玻璃質感 (`360x56` px，`backdrop-filter: blur(16px)`)，常駐於簡報畫面邊緣，不遮擋核心簡報內容。
* **雙向即時感知**：簡報換頁事件透過本機 WebSocket / WebAPI 於 **< 16ms** 內即時同步至浮動工具列，即時更新頁碼 `[05/24]`、標題與錄音時長。
* **系統級全螢幕穿透**：macOS 調用 `NSWindowCollectionBehaviorFullScreenAuxiliary` 與所有 Spaces 行為；Windows 於失焦時智慧重新確認置頂。

### 2. Slide 原子分片與不可變累加 (Atomic Sharding & No-Clobber)
* **投影片物理隔離**：音訊以 `slide_id` 為原子最小目錄獨立儲存（`audio/<slide_id>/take_N.wav`）。
* **歷史版本不滅**：重錄新版本永遠是單調遞增追加（Take 01, Take 02, Take 03...），嚴禁覆蓋既有錄音資產。講者永遠保有歷史備份，心態徹底放鬆。

### 3. 連續演練換頁自動切檔 (Seamless Auto-Chunking)
* **講者一氣呵成暢講**：講者在第 1 頁按下錄音後，可照常一口氣演練講完全部投影片。
* **無感背景切片**：講者每按一次簡報筆或鍵盤換頁（`→` / `Space`），麥克風保持持續收音，底層自動封裝前一頁 WAV 並零延遲開啟新頁錄音緩衝，實現「演練像平常一樣順暢，錄完自動獲得全套分頁 Take 卡片」。

### 4. 進階矩陣工作室與頂部頁籤 (Studio Matrix & Top Tab Navigation)
* **全域掌控棋盤**：點擊放大按鈕即刻開啟深色不透明之 1080x720 工作台，自訂標題列支援跨平台拖曳、最小化與最大化。
* **頂部頁籤導航**：
  * **【🎙️ 錄音編輯】**：左側展示全簡報投影片清單（頁碼、標題、綠/灰色錄音狀態圓點、備忘稿）；右側橫排展開 **Take 01 ～ Take 05** 矩陣網格，支援波形試聽、直接標註 **★ 黃金錄音** 或刪除。
  * **【⚙️ 系統設定】**：整合 Global 11 語言切換與可配置式簡報解析引擎。

### 5. 可配置式 JSON 簡報解析引擎 (Configurable JSON Deck Parser)
* **開箱即用支援**：內建原生支援 Reveal.js (`<section>`)、Marp (`.marp-slide`)、Slidev、通用語意 H1/H2 標題與 PowerPoint (.pptx) OpenXML 靜態解構。
* **自訂 JSON 規則**：提供多行程式碼文字編輯方塊，使用者可填入自訂 `slide_selector`、`title_selector` 與 `notes_selector`，支援 `.json` 檔案載入與匯出。
* **即時測試預覽**：提供「▶ 測試解析當前簡報」按鈕，0.1 秒內即時反饋辨識出的總頁數與標題預覽，杜絕盲測。

### 6. 無失真母帶轉檔與 15 秒 4K 教學影片合成 (Master Audio & Video Synthesis)
* **一鍵母帶轉檔**：讀取 `slidetake-manifest.json`，依序將各頁 Golden Take 音軌平滑交叉淡入（Cross-fade）串接，一鍵輸出高品質完整音訊母帶（WAV / MP3）。
* **秒級影音離線合流**：透過無頭瀏覽器擷取高解析度投影片圖片，時序嚴格對齊各頁 Golden Take 時長，經 FFmpeg 離線合成管線輸出 H.264/AAC 4K MP4，1 小時課程僅需 15~20 秒即可完成，徹底告別耗時剪輯！

---

## 系統架構總覽 (Architecture Overview)

SAISlideTake 採用模組化單體（Modular Monolith）與本地優先（Local-First）設計：

```text
┌────────────────────────────────────────────────────────────────────────┐
│                        展示與客戶端層 (Presentation)                    │
│  ┌──────────────────────────────────┐  ┌────────────────────────────┐  │
│  │     Tauri v2 Desktop Shell       │  │   Vue 3 Web Presentation   │  │
│  │   (Floating Capsule / Studio)    │  │ (Reveal.js / Marp / Slidev)│  │
│  └──────────────────┬───────────────┘  └─────────────┬──────────────┘  │
└─────────────────────┼────────────────────────────────┼─────────────────┘
                      │ (Local REST / WebSocket :3188) │
┌─────────────────────▼────────────────────────────────▼─────────────────┐
│              saist-core (Rust 模組化單體本機服務平面)                   │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │         Axum HTTP / WebSocket 狀態總線 & Deck Ingestion          │  │
│  └─────────────────┬───────────────────────────────┬────────────────┘  │
│                    │                               │                   │
│  ┌─────────────────▼──────────────┐  ┌─────────────▼────────────────┐  │
│  │ CPAL Native PCM 錄音串流緩衝   │  │ DeckParser (HTML / OpenXML)  │  │
│  └─────────────────┬──────────────┘  └─────────────┬────────────────┘  │
│                    │                               │                   │
│  ┌─────────────────▼──────────────┐  ┌─────────────▼────────────────┐  │
│  │ Audio Storage & Atomic Sharding│  │ FFmpeg Concat 影音合成引擎   │  │
│  └─────────────────┬──────────────┘  └─────────────┬────────────────┘  │
└────────────────────┼───────────────────────────────┼───────────────────┘
                     │                               │
┌────────────────────▼───────────────────────────────▼───────────────────┐
│                   簡報專案實體磁碟區 (Local Presentation Root)          │
│  ├── presentation.html (或 .pptx)                                      │
│  └── audio/                                                            │
│      ├── slidetake-manifest.json (黃金音軌與版本映射憲法)               │
│      ├── slide_01_Introduction/take_01.wav, take_02.wav               │
│      └── slide_02_Architecture/take_01.wav                            │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 專案結構 (Project Structure)

```text
SAISlideTake/
├── 9.output/                     # 統一建置與發行產物收斂目錄 (嚴禁污染原始碼)
├── docs/                         # 使用手冊
│   ├── USER_GUIDE.md             # 使用手冊（英文）
│   └── USER_GUIDE.zh-TW.md       # 使用手冊（繁體中文）
├── proto/                        # Protobuf / gRPC 通訊契約 (slide_take.proto)
├── src/
│   ├── saist-core/               # Rust 本機核心 (Axum, CPAL, DeckParser, AudioStorage)
│   ├── saist-portal/             # Tauri 桌面端 (原生視窗穿透、全螢幕輔助、拖放事件)
│   └── saist-web/                # Vue 3 前端 (膠囊工具列、矩陣工作室、11 國語系)
│       └── public/
│           └── sai-slidetake-bridge.js # 簡報端雙向通訊 SDK
├── scripts/                      # 跨平台除錯與建置自動化腳本 (macos/, win/)
└── Cargo.toml                    # Rust 模組工作區配置
```

---

## 快速開始 (Quick Start)

### 系統環境要求 (Prerequisites)
- **Rust**: 1.85+ (`cargo`, `rustc`)
- **Node.js**: 20+ (`npm`)
- **FFmpeg**：免費開源元件；僅母帶合併與教學影片合成時需要（日常錄音免裝），macOS（`brew install ffmpeg`）與 Windows（`winget install Gyan.FFmpeg` 或設定 `SAIST_FFMPEG`）皆須預先安裝

### 啟動開發環境 (Development Mode)

```bash
# 1. 安裝前端依賴
cd src/saist-web && npm install && cd ../..

# 2. 使用跨平台除錯腳本啟動 HMR
# macOS:
./scripts/macos/SAISlideTake_DEBUG.sh

# Windows:
.\scripts\win\SAISlideTake_DEBUG.bat
```

### 專案品質驗收命令 (Verification Gates)

```bash
# 驗證 Rust 後端單元測試與編譯
cargo test --workspace
cargo check --workspace

# 驗證前端 TypeScript 型別與 Production 建置
cd src/saist-web
npm run typecheck
npm run build
```

---

## 授權條款 (License)

本專案採用 **[MIT License](LICENSE)** 授權條款開源發布。您可以自由修改、散布與商業使用，詳見 LICENSE 檔案說明。
