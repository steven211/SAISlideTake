(function () {
  "use strict";

  if (document.getElementById("sai-slidetake-embedded-host")) return;

  const API = "http://127.0.0.1:3188";
  const locale = (navigator.language || "en").toLowerCase();
  const language = locale.startsWith("zh-tw") ? "zh-TW"
    : locale.startsWith("zh") ? "zh-CN"
      : locale.startsWith("ja") ? "ja-JP"
        : locale.startsWith("ko") ? "ko-KR"
          : locale.startsWith("fr") ? "fr-FR"
            : locale.startsWith("de") ? "de-DE"
              : locale.startsWith("pt") ? "pt-BR"
                : locale.startsWith("ru") ? "ru-RU"
                  : locale.startsWith("vi") ? "vi-VN"
                    : locale.startsWith("es") ? "es-ES" : "en-US";
  const strings = {
    "zh-TW": { record: "錄音", stop: "停止", replay: "重聽黃金版", takes: "版本", studio: "工作室", overlay: "原生浮窗", hideOverlay: "隱藏原生浮窗", setGolden: "設為黃金", play: "試聽", empty: "尚無錄音版本", waiting: "等待簡報連線", saved: "錄音已儲存", failed: "操作失敗，請確認 SAISlideTake 正在執行。", title: "SAISlideTake" },
    "zh-CN": { record: "录音", stop: "停止", replay: "重听黄金版", takes: "版本", studio: "工作室", overlay: "原生浮窗", hideOverlay: "隐藏原生浮窗", setGolden: "设为黄金", play: "试听", empty: "尚无录音版本", waiting: "等待简报连接", saved: "录音已保存", failed: "操作失败，请确认 SAISlideTake 正在运行。", title: "SAISlideTake" },
    "en-US": { record: "Record", stop: "Stop", replay: "Replay golden", takes: "Takes", studio: "Studio", overlay: "Native bar", hideOverlay: "Hide native bar", setGolden: "Set golden", play: "Play", empty: "No takes yet", waiting: "Waiting for presentation", saved: "Recording saved", failed: "Action failed. Check that SAISlideTake is running.", title: "SAISlideTake" },
    "ja-JP": { record: "録音", stop: "停止", replay: "ゴールデンを再生", takes: "テイク", studio: "スタジオ", overlay: "ネイティブバー", hideOverlay: "ネイティブバーを隠す", setGolden: "ゴールデンに設定", play: "再生", empty: "録音はありません", waiting: "プレゼンを待機中", saved: "録音を保存しました", failed: "操作に失敗しました。SAISlideTake の起動を確認してください。", title: "SAISlideTake" },
    "ko-KR": { record: "녹음", stop: "중지", replay: "골든 다시 듣기", takes: "테이크", studio: "스튜디오", overlay: "기본 도구 모음", hideOverlay: "기본 도구 모음 숨기기", setGolden: "골든으로 설정", play: "재생", empty: "녹음이 없습니다", waiting: "프레젠테이션 연결 대기 중", saved: "녹음 저장됨", failed: "작업 실패. SAISlideTake 실행을 확인하세요.", title: "SAISlideTake" },
    "es-ES": { record: "Grabar", stop: "Detener", replay: "Reproducir favorita", takes: "Tomas", studio: "Estudio", overlay: "Barra nativa", hideOverlay: "Ocultar barra nativa", setGolden: "Marcar favorita", play: "Reproducir", empty: "Aún no hay tomas", waiting: "Esperando presentación", saved: "Grabación guardada", failed: "Error. Comprueba que SAISlideTake esté en ejecución.", title: "SAISlideTake" },
    "fr-FR": { record: "Enregistrer", stop: "Arrêter", replay: "Écouter la préférée", takes: "Prises", studio: "Studio", overlay: "Barre native", hideOverlay: "Masquer la barre native", setGolden: "Définir comme préférée", play: "Lire", empty: "Aucun enregistrement", waiting: "En attente de la présentation", saved: "Enregistrement sauvegardé", failed: "Échec. Vérifiez que SAISlideTake est en cours.", title: "SAISlideTake" },
    "de-DE": { record: "Aufnehmen", stop: "Stopp", replay: "Favorit abspielen", takes: "Takes", studio: "Studio", overlay: "Native Leiste", hideOverlay: "Native Leiste ausblenden", setGolden: "Als Favorit markieren", play: "Abspielen", empty: "Noch keine Aufnahmen", waiting: "Warte auf Präsentation", saved: "Aufnahme gespeichert", failed: "Fehler. Prüfen Sie, ob SAISlideTake ausgeführt wird.", title: "SAISlideTake" },
    "pt-BR": { record: "Gravar", stop: "Parar", replay: "Reproduzir favorita", takes: "Takes", studio: "Estúdio", overlay: "Barra nativa", hideOverlay: "Ocultar barra nativa", setGolden: "Definir como favorita", play: "Reproduzir", empty: "Nenhuma gravação", waiting: "Aguardando apresentação", saved: "Gravação salva", failed: "Falha. Verifique se o SAISlideTake está em execução.", title: "SAISlideTake" },
    "ru-RU": { record: "Запись", stop: "Стоп", replay: "Повтор избранного", takes: "Дубли", studio: "Студия", overlay: "Родная панель", hideOverlay: "Скрыть родную панель", setGolden: "Сделать избранным", play: "Воспроизвести", empty: "Записей пока нет", waiting: "Ожидание презентации", saved: "Запись сохранена", failed: "Ошибка. Проверьте, запущен ли SAISlideTake.", title: "SAISlideTake" },
    "vi-VN": { record: "Ghi âm", stop: "Dừng", replay: "Phát bản vàng", takes: "Bản thu", studio: "Studio", overlay: "Thanh gốc", hideOverlay: "Ẩn thanh gốc", setGolden: "Đặt làm bản vàng", play: "Phát", empty: "Chưa có bản thu", waiting: "Đang chờ bản trình chiếu", saved: "Đã lưu bản ghi", failed: "Thao tác thất bại. Hãy kiểm tra SAISlideTake đang chạy.", title: "SAISlideTake" },
  }[language];

  const host = document.createElement("div");
  host.id = "sai-slidetake-embedded-host";
  host.style.cssText = "position:fixed;inset:0;width:100%;height:100%;z-index:2147483647;pointer-events:none;";
  const shadow = host.attachShadow({ mode: "open" });
  shadow.innerHTML = `
    <style>
      :host { all: initial; }
      * { box-sizing: border-box; }
      .panel { position:absolute; top:18px; right:18px; width:min(390px,calc(100vw - 24px)); color:#eef5ff; font:13px/1.4 -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif; pointer-events:auto; }
      .bar { min-height:58px; display:flex; align-items:center; gap:8px; padding:8px 10px; border:1px solid #ffffff28; border-radius:18px; background:rgba(13,25,43,.88); box-shadow:0 8px 32px #0008; backdrop-filter:blur(14px); -webkit-backdrop-filter:blur(14px); cursor:grab; touch-action:none; user-select:none; }
      .bar.dragging { cursor:grabbing; }
      .identity { flex:1; min-width:0; cursor:grab; }
      .title { display:block; overflow:hidden; color:#f8fbff; font-weight:650; text-overflow:ellipsis; white-space:nowrap; }
      .subtitle { display:flex; align-items:center; gap:6px; margin-top:2px; color:#9eafc4; font-size:11px; }
      .dot { width:7px; height:7px; border-radius:50%; background:#5c7188; }
      .dot.online { background:#31d49b; box-shadow:0 0 8px #31d49b; }
      .dot.recording { background:#ff5364; box-shadow:0 0 9px #ff5364; animation:pulse 1s infinite; }
      button { min-height:34px; border:1px solid #ffffff20; border-radius:11px; padding:0 10px; color:#e6f0ff; background:#ffffff0b; font:inherit; cursor:pointer; white-space:nowrap; }
      button:hover { background:#ffffff1b; }
      button:disabled { opacity:.42; cursor:default; }
      .record { border-color:#ff667444; color:#ff8b96; }
      .record.active { color:white; background:#c72e46; }
      .icon { width:35px; padding:0; font-size:16px; }
      .drawer { display:none; max-height:min(48vh,390px); margin-top:8px; overflow:auto; border:1px solid #ffffff24; border-radius:15px; padding:8px; background:rgba(12,23,39,.94); box-shadow:0 8px 32px #0008; backdrop-filter:blur(14px); }
      .drawer.open { display:block; }
      .drawer-head { display:flex; justify-content:space-between; align-items:center; padding:4px 4px 8px; color:#b8c8dc; font-size:12px; }
      .take { display:flex; align-items:center; gap:7px; margin:5px 0; padding:8px; border:1px solid #ffffff12; border-radius:11px; background:#ffffff08; }
      .take.golden { border-color:#f6b94c80; }
      .take-info { flex:1; min-width:0; }
      .take-name { overflow:hidden; color:#edf4ff; text-overflow:ellipsis; white-space:nowrap; }
      .take-meta { color:#96a9bf; font-size:10px; }
      .take button { min-height:30px; padding:0 8px; font-size:11px; }
      .golden-mark { color:#ffcb5d; }
      .empty { padding:16px 8px; color:#99aabe; text-align:center; }
      .toast { position:absolute; top:calc(100% + 8px); right:3px; max-width:100%; padding:7px 10px; border-radius:9px; color:#fff; background:#151e2ddf; box-shadow:0 4px 20px #0005; }
      @keyframes pulse { 50% { opacity:.45; } }
      @media (max-width:520px) { .panel { top:8px; right:8px; } .bar { gap:5px; padding:7px; } button { padding:0 7px; } .icon { width:32px; } }
    </style>
    <section class="panel" aria-label="${strings.title}">
      <div class="bar">
        <div class="identity">
          <span class="title" data-ref="title">${strings.waiting}</span>
          <span class="subtitle"><i class="dot" data-ref="dot"></i><span data-ref="status">${strings.waiting}</span></span>
        </div>
        <button class="record" data-action="record" title="${strings.record}">●</button>
        <button class="icon" data-action="replay" title="${strings.replay}">▶</button>
        <button data-action="takes" title="${strings.takes}">▤ <span data-ref="take-count">0</span></button>
        <button class="icon" data-action="studio" title="${strings.studio}">▦</button>
        <button class="icon" data-action="overlay" title="${strings.overlay}">⧉</button>
      </div>
      <div class="drawer" data-ref="drawer"><div class="drawer-head"><span>${strings.takes}</span><button data-action="close-takes" aria-label="×">×</button></div><div data-ref="take-list"></div></div>
      <div class="toast" data-ref="toast" hidden></div>
    </section>`;

  const $ = (selector) => shadow.querySelector(selector);
  const titleNode = $('[data-ref="title"]');
  const panel = $(".panel");
  const bar = $(".bar");
  const statusNode = $('[data-ref="status"]');
  const dotNode = $('[data-ref="dot"]');
  const recordButton = $('[data-action="record"]');
  const drawer = $('[data-ref="drawer"]');
  const takeList = $('[data-ref="take-list"]');
  const toast = $('[data-ref="toast"]');
  let slide = window.SAISlideTakeBridge?.getActiveSlideInfo?.() || null;
  let group = null;
  let recording = false;
  let recordingSeconds = 0;
  let nativeVisible = false;
  let statusTimer = 0;
  let toastTimer = 0;
  let currentAudio = null;
  let drawerOpen = false;
  let dragState = null;

  function bridgeSend(type, payload) {
    const sent = window.SAISlideTakeBridge?.send?.(type, payload || {});
    if (!sent) notify(strings.failed);
  }

  async function request(path, init) {
    const response = await fetch(`${API}${path}`, init);
    if (!response.ok) {
      const result = await response.json().catch(() => ({}));
      throw new Error(result.error || `HTTP ${response.status}`);
    }
    return response.json();
  }

  function formatTime(seconds) {
    const value = Math.max(0, Math.floor(Number(seconds) || 0));
    return `${String(Math.floor(value / 60)).padStart(2, "0")}:${String(value % 60).padStart(2, "0")}`;
  }

  function notify(message) {
    toast.textContent = message;
    toast.hidden = false;
    window.clearTimeout(toastTimer);
    toastTimer = window.setTimeout(() => { toast.hidden = true; }, 2600);
  }

  function render() {
    const current = slide;
    titleNode.textContent = current
      ? `${String(current.slide_index + 1).padStart(2, "0")}/${current.total_slides} · ${current.slide_title}`
      : strings.waiting;
    statusNode.textContent = recording ? `${strings.stop} · ${formatTime(recordingSeconds)}` : (current ? strings.title : strings.waiting);
    dotNode.className = `dot${recording ? " recording" : current ? " online" : ""}`;
    recordButton.textContent = recording ? `■ ${formatTime(recordingSeconds)}` : "●";
    recordButton.title = recording ? strings.stop : strings.record;
    recordButton.classList.toggle("active", recording);
    recordButton.disabled = !current;
    const takes = group?.takes || [];
    $('[data-ref="take-count"]').textContent = String(takes.length);
    $('[data-action="overlay"]').title = nativeVisible ? strings.hideOverlay : strings.overlay;
    $('[data-action="overlay"]').setAttribute("aria-label", nativeVisible ? strings.hideOverlay : strings.overlay);
    if (drawerOpen) renderTakes();
  }

  function renderTakes() {
    takeList.replaceChildren();
    const takes = (group?.takes || []).slice().sort((a, b) => String(b.created_at).localeCompare(String(a.created_at)));
    if (!takes.length) {
      const empty = document.createElement("div");
      empty.className = "empty";
      empty.textContent = strings.empty;
      takeList.appendChild(empty);
      return;
    }
    for (const take of takes) {
      const row = document.createElement("div");
      row.className = `take${take.take_id === group.golden_take_id ? " golden" : ""}`;
      const info = document.createElement("div");
      info.className = "take-info";
      const name = document.createElement("div");
      name.className = "take-name";
      name.textContent = `${take.take_id}${take.take_id === group.golden_take_id ? " ★" : ""}`;
      const meta = document.createElement("div");
      meta.className = "take-meta";
      meta.textContent = `${formatTime(take.duration_sec)} · ${new Date(take.created_at).toLocaleString()}`;
      info.append(name, meta);
      const play = document.createElement("button");
      play.textContent = "▶";
      play.title = strings.play;
      play.addEventListener("click", () => playTake(take));
      row.append(info, play);
      if (take.take_id !== group.golden_take_id) {
        const golden = document.createElement("button");
        golden.className = "golden-mark";
        golden.textContent = "★";
        golden.title = strings.setGolden;
        golden.addEventListener("click", () => setGolden(take.take_id));
        row.appendChild(golden);
      }
      takeList.appendChild(row);
    }
  }

  async function refreshTakes() {
    if (!slide?.slide_id) { group = null; render(); return; }
    try {
      group = await request(`/api/slides/${encodeURIComponent(slide.slide_id)}/takes`);
      render();
    } catch (_) { /* Keep the last visible Take list during a brief Core reconnect. */ }
  }

  async function setGolden(takeId) {
    if (!slide?.slide_id) return;
    try {
      group = await request(`/api/slides/${encodeURIComponent(slide.slide_id)}/golden`, {
        method: "PUT", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ take_id: takeId }),
      });
      render();
    } catch (_) { notify(strings.failed); }
  }

  function playTake(take) {
    if (!take) return;
    if (currentAudio) { currentAudio.pause(); currentAudio.src = ""; }
    currentAudio = new Audio(`${API}/api/audio/${encodeURIComponent(take.slide_id)}/${encodeURIComponent(take.filename)}`);
    currentAudio.onerror = () => notify(strings.failed);
    currentAudio.play().catch(() => notify(strings.failed));
  }

  function replayGolden() {
    const take = group?.takes?.find((item) => item.take_id === group.golden_take_id);
    if (take) playTake(take);
    else notify(strings.empty);
  }

  async function startRecording() {
    if (!slide?.slide_id) return;
    try {
      const status = await request("/api/recording/start", {
        method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ slide_id: slide.slide_id }),
      });
      recording = Boolean(status.active);
      recordingSeconds = Number(status.elapsed_sec) || 0;
      window.clearInterval(statusTimer);
      statusTimer = window.setInterval(updateRecordingStatus, 500);
      render();
    } catch (error) { notify(error?.message || strings.failed); }
  }

  async function stopRecording() {
    recording = false;
    window.clearInterval(statusTimer);
    render();
    try {
      group = await request("/api/recording/stop", { method: "POST" });
      recordingSeconds = 0;
      render();
      notify(strings.saved);
    } catch (error) { notify(error?.message || strings.failed); }
  }

  async function updateRecordingStatus() {
    try {
      const status = await request("/api/recording/status");
      recording = Boolean(status.active);
      recordingSeconds = Number(status.elapsed_sec) || 0;
      render();
      if (!recording) {
        window.clearInterval(statusTimer);
        void refreshTakes();
      }
    } catch (_) { /* Retry while Core is reconnecting. */ }
  }

  function onBridgeEvent(event) {
    const message = event.detail || {};
    if (message.type === "slide_changed") {
      slide = message.payload || slide;
      group = null;
      render();
      void refreshTakes();
    } else if (["takes_updated", "golden_take_updated", "take_deleted"].includes(message.type)) {
      if (message.payload?.slide_id === slide?.slide_id) {
        group = message.payload;
        render();
      }
    } else if (message.type === "toggle_native_overlay") {
      nativeVisible = Boolean(message.payload?.visible);
      render();
    }
  }

  function moveToFullscreenRoot() {
    const target = document.fullscreenElement || document.documentElement;
    if (host.parentElement !== target) target.appendChild(host);
  }

  function clampPanelPosition(left, top) {
    const rect = panel.getBoundingClientRect();
    return {
      left: Math.max(0, Math.min(window.innerWidth - rect.width, left)),
      top: Math.max(0, Math.min(window.innerHeight - rect.height, top)),
    };
  }

  function restorePanelPosition() {
    try {
      const saved = JSON.parse(window.localStorage.getItem("sai-slidetake-embedded-position") || "null");
      if (!saved || !Number.isFinite(saved.x) || !Number.isFinite(saved.y)) return;
      requestAnimationFrame(() => {
        const rect = panel.getBoundingClientRect();
        const position = clampPanelPosition(
          saved.x * Math.max(0, window.innerWidth - rect.width),
          saved.y * Math.max(0, window.innerHeight - rect.height),
        );
        panel.style.left = `${position.left}px`;
        panel.style.top = `${position.top}px`;
        panel.style.right = "auto";
      });
    } catch (_) { /* Private browsing may disable localStorage. */ }
  }

  function beginPanelDrag(event) {
    const target = event.target instanceof Element ? event.target : null;
    if (event.button !== 0 || !target?.closest(".bar") || target.closest("button")) return;
    const rect = panel.getBoundingClientRect();
    panel.style.left = `${rect.left}px`;
    panel.style.top = `${rect.top}px`;
    panel.style.right = "auto";
    dragState = { pointerId: event.pointerId, x: event.clientX, y: event.clientY, left: rect.left, top: rect.top };
    bar.classList.add("dragging");
    try { bar.setPointerCapture(event.pointerId); } catch (_) { /* Continue with bubbling pointer events. */ }
    event.preventDefault();
  }

  function movePanelDrag(event) {
    if (!dragState || event.pointerId !== dragState.pointerId) return;
    const position = clampPanelPosition(
      dragState.left + event.clientX - dragState.x,
      dragState.top + event.clientY - dragState.y,
    );
    panel.style.left = `${position.left}px`;
    panel.style.top = `${position.top}px`;
  }

  function endPanelDrag(event) {
    if (!dragState || event.pointerId !== dragState.pointerId) return;
    const rect = panel.getBoundingClientRect();
    const maxLeft = Math.max(1, window.innerWidth - rect.width);
    const maxTop = Math.max(1, window.innerHeight - rect.height);
    try {
      window.localStorage.setItem("sai-slidetake-embedded-position", JSON.stringify({
        x: Math.max(0, Math.min(1, rect.left / maxLeft)),
        y: Math.max(0, Math.min(1, rect.top / maxTop)),
      }));
    } catch (_) { /* Keep the current position for this page if storage is unavailable. */ }
    dragState = null;
    bar.classList.remove("dragging");
  }

  function keepPanelInViewport() {
    const rect = panel.getBoundingClientRect();
    if (!panel.style.left) return;
    const position = clampPanelPosition(rect.left, rect.top);
    panel.style.left = `${position.left}px`;
    panel.style.top = `${position.top}px`;
  }

  shadow.addEventListener("click", (event) => {
    const button = event.target.closest("button[data-action]");
    if (!button) return;
    switch (button.dataset.action) {
      case "record": recording ? void stopRecording() : void startRecording(); break;
      case "replay": replayGolden(); break;
      case "takes":
        drawerOpen = !drawerOpen;
        drawer.classList.toggle("open", drawerOpen);
        if (drawerOpen) void refreshTakes();
        break;
      case "close-takes":
        drawerOpen = false;
        drawer.classList.remove("open");
        break;
      case "studio": bridgeSend("open_native_studio"); break;
      case "overlay": bridgeSend("toggle_native_overlay", { visible: !nativeVisible }); break;
    }
  });
  shadow.addEventListener("pointerdown", beginPanelDrag);
  shadow.addEventListener("pointermove", movePanelDrag);
  shadow.addEventListener("pointerup", endPanelDrag);
  shadow.addEventListener("pointercancel", endPanelDrag);

  window.addEventListener("sai-slidetake-event", onBridgeEvent);
  document.addEventListener("fullscreenchange", moveToFullscreenRoot);
  window.addEventListener("resize", keepPanelInViewport);
  window.addEventListener("beforeunload", () => {
    window.clearInterval(statusTimer);
    window.clearTimeout(toastTimer);
    currentAudio?.pause();
    window.removeEventListener("sai-slidetake-event", onBridgeEvent);
    document.removeEventListener("fullscreenchange", moveToFullscreenRoot);
    window.removeEventListener("resize", keepPanelInViewport);
  }, { once: true });

  moveToFullscreenRoot();
  restorePanelPosition();
  render();
  void refreshTakes();
  void updateRecordingStatus();
})();
