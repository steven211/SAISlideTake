# SAISlideTake Gitea Actions 雙平台編譯與發行指南

本目錄提供 Mac mini 與 Windows MiniPC 的 Gitea `act_runner` 一鍵註冊與啟動腳本，搭配 [release.yml](../../.gitea/workflows/release.yml) 可達成「**一次觸發 → 雙機平行編譯 → 自動發佈 Gitea & GitHub Releases**」。

---

## 1. 在 Gitea 取得 Runner Registration Token
1. 登入 Gitea (`http://github.com`)。
2. 進入專案 `steven211/SAISlideTake` → **設定 (Settings)** → **Actions** → **Runners**（或全站管理後台的 Runners）。
3. 點擊 **Create new Runner**，複製產生的 **Registration Token**。

---

## 2. 在兩台編譯機註冊並啟動 Runner

### Mac mini (macOS arm64)
```bash
chmod +x scripts/ci/setup-gitea-runner-macos.sh
./scripts/ci/setup-gitea-runner-macos.sh "<YOUR_RUNNER_TOKEN>"
```
> 若要背景常駐，可執行 `nohup act_runner daemon > ~/.gitea-runner/runner.log 2>&1 &`（在 `~/.gitea-runner` 目錄下執行）或透過 `launchd` 管理。

### MiniPC (Windows x64)
```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\ci\setup-gitea-runner-win.ps1 -Token "<YOUR_RUNNER_TOKEN>"
```
> 建議以系統管理員或開機工作排程器常駐執行 `C:\Users\<帳號>\.gitea-runner\act_runner.exe daemon`。

---

## 3. 設定 GitHub 自動發行權限（選用）
若要讓 Workflow 編譯完成後自動將壓縮檔上傳至 GitHub (`steven211/SAISlideTake`) Releases：
1. 到 GitHub 建立一組 Fine-grained Personal Access Token（僅授權 `steven211/SAISlideTake` 的 `Contents: Read and write`）。
2. 到 Gitea 專案 → **設定 (Settings)** → **Secrets** → **Actions** → 新增 Secret：
   - Name: `GH_TOKEN`
   - Value: 貼上剛才產生的 GitHub Token
> 若未設定 `GH_TOKEN`，Workflow 仍會正常編譯並發佈到 **Gitea Releases**，僅會跳過 GitHub 上傳步驟。

---

## 4. 觸發雙平台建置與發行
- **方式一（Tag 自動觸發）**：更新 `src/saist-portal/Cargo.toml` 的 `version` 後，推上對應 Tag：
  ```bash
  git tag v0.1.0
  git push origin v0.1.0
  ```
- **方式二（手動一鍵觸發）**：到 Gitea 專案 **Actions** 頁籤 → 選擇 **SAISlideTake Cross-Platform Release** → 點擊 **Run workflow**。
