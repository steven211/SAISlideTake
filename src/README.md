# SAISlideTake Core Source Architecture (`src/`)

[English](README.md) | [繁體中文](README.zh-TW.md)

```text
src/
├── saist-core/     # [Backend] Rust core engine, audio sharding, manifest manager, and WebAPI/WebSocket bridge (:3188)
├── saist-web/      # [Frontend] Vue 3 + Vite + TypeScript floating overlay widget, Take Cards drawer, and presentation bridge SDK
└── saist-portal/   # [Desktop] Tauri v2 frameless always-on-top desktop shell
```
