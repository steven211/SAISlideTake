# SAISlideTake 核心原始碼架構 (`src/`)

[English](README.md) | [繁體中文](README.zh-TW.md)

本目錄為 **SAISlideTake** 的核心原始碼收斂目錄，比照 `SAIAgent` 與 `SAIQueryStorePortal` 模組化單體架構組織為三大核心專案組件：

```text
src/
├── saist-core/     # [後端核心] Rust 音訊分片引擎、slidetake-manifest.json 管理與 WebAPI/WebSocket 雙向橋接服務 (:3188)
├── saist-web/      # [前端視圖] Vue 3 + Vite + TypeScript 置頂浮動工具列、版本卡片抽屜與簡報嵌入橋接腳本 (sai-slidetake-bridge.js)
└── saist-portal/   # [桌面外殼] Rust + Tauri v2 無邊框置頂桌面應用外殼與原生系統整合
```
