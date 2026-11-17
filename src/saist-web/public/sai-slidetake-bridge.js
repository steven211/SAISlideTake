(function () {
  "use strict";

  const API = "http://127.0.0.1:3188";
  const SOCKET = "ws://127.0.0.1:3188/ws";
  function collectSlides(profile) {
    if (profile?.slide_selector) {
      try {
        const selected = Array.from(document.querySelectorAll(profile.slide_selector));
        if (selected.length) {
          if (String(profile.engine_name || "").toLowerCase().includes("reveal")) {
            const leaves = [];
            const append = (element) => {
              const children = Array.from(element.children).filter((child) => child.matches(profile.slide_selector) || child.tagName.toLowerCase() === "section");
              if (children.length) children.forEach(append);
              else leaves.push(element);
            };
            selected.filter((element) => !element.parentElement?.closest(profile.slide_selector)).forEach(append);
            return leaves;
          }
          return selected.filter((slide) => !slide.parentElement?.closest(profile.slide_selector));
        }
        if (profile.fallback_strategy === "heading_h1") return Array.from(document.querySelectorAll("h1"));
        if (profile.fallback_strategy === "heading_h2") return Array.from(document.querySelectorAll("h2"));
        if (profile.fallback_strategy === "hr_split") return Array.from(document.querySelectorAll("h1, h2"));
      } catch (error) {
        console.warn("[SAISlideTake] Invalid slide selector:", error);
        return [];
      }
      return [];
    }
    const revealRoots = Array.from(document.querySelectorAll(".slides > section"));
    const revealSlides = [];
    const appendReveal = (element) => {
      const children = Array.from(element.children).filter((child) => child.tagName.toLowerCase() === "section");
      if (children.length) children.forEach(appendReveal);
      else revealSlides.push(element);
    };
    if (revealRoots.length) {
      revealRoots.forEach(appendReveal);
      return revealSlides;
    }
    const marpSlides = Array.from(document.querySelectorAll(".marp-slide, section[data-marpit-pagination], section[data-marpit-pagination-total]"));
    if (marpSlides.length) return marpSlides;
    return Array.from(document.querySelectorAll(".slide, .chapter")).filter((slide) =>
      !slide.parentElement?.closest(".slide, .chapter"));
  }
  let activeProfile = window.__SAISLIDETAKE_PARSER_PROFILE__ || null;
  let slides = [];
  let slideIds = [];
  let stateObserver = null;
  function slideTitle(slide, index) {
    if (!slide) return `第 ${index + 1} 頁`;
    const profileSelector = activeProfile?.title_selector;
    let heading;
    try { heading = profileSelector ? slide.querySelector(profileSelector) : slide.querySelector("h1, h2, .slide-title"); } catch (_) { heading = null; }
    const ownHeading = ["h1", "h2", "h3"].includes(slide.tagName.toLowerCase()) ? slide.textContent?.trim() : "";
    return slide.getAttribute("data-title") || heading?.getAttribute("data-title") || heading?.textContent?.trim() || ownHeading || `第 ${index + 1} 頁`;
  }
  function rebuildSlideIndex(profile) {
    activeProfile = profile || null;
    slides = collectSlides(activeProfile);
    const usedSlideIds = new Set();
    slideIds = slides.map((slide, index) => {
      const title = slideTitle(slide, index);
      const slug = title.replace(/[\s/\\]+/g, "_").slice(0, 72) || `slide_${String(index + 1).padStart(2, "0")}`;
      const splitFallback = activeProfile?.fallback_strategy === "hr_split" && !slide.matches(activeProfile.slide_selector);
      const raw = splitFallback ? `slide_${String(index + 1).padStart(2, "0")}_${slug}` : slide.getAttribute("data-slide-id") || slide.id || `slide_${String(index + 1).padStart(2, "0")}_${slug}`;
      const base = Array.from(raw).filter((character) => !/[\\/:]/.test(character) && character.charCodeAt(0) >= 32).slice(0, 140).join("") || `slide_${String(index + 1).padStart(2, "0")}`;
      let result = base;
      let suffix = 2;
      while (usedSlideIds.has(result)) result = `${base}_${suffix++}`;
      usedSlideIds.add(result);
      return result;
    });
    if (stateObserver) watchActiveState();
  }
  function watchActiveState() {
    if (!stateObserver) return;
    const attributes = new Set(["class", "aria-hidden", "aria-current"]);
    String(activeProfile?.active_class || "").split(",").map((token) => token.trim()).forEach((token) => {
      const match = /^\[\s*([^\s~|^$*!=\]]+)/.exec(token);
      if (match) attributes.add(match[1]);
    });
    stateObserver.disconnect();
    stateObserver.observe(document.documentElement, { subtree: true, attributes: true, attributeFilter: Array.from(attributes) });
  }
  rebuildSlideIndex(activeProfile);
  let socket = null;
  let reconnectTimer = 0;
  let lastSlideKey = "";
  let slideUpdateQueue = Promise.resolve();
  let autoplayToken = 0;
  let autoplayAudio = null;
  let isAutoplaying = false;
  let html2canvasPromise = null;

  function getActiveSlideInfo() {
    const activeTokens = String(activeProfile?.active_class || "active, present, current, bespoke-marp-active").split(",").map((token) => token.trim()).filter(Boolean);
    const activeIndex = slides.findIndex((slide) => activeTokens.some((token) => {
      if (token.startsWith(".") || token.startsWith("#") || token.startsWith("[")) {
        try { return slide.matches(token); } catch (_) { return false; }
      }
      return slide.classList.contains(token);
    }) || slide.getAttribute("aria-hidden") === "false");
    const index = activeIndex >= 0 ? activeIndex : 0;
    const activeSlide = slides[index];
    const title = slideTitle(activeSlide, index);
    const safeTitle = title.replace(/[\s/\\]+/g, "_").slice(0, 72) || `slide_${String(index + 1).padStart(2, "0")}`;
    return {
      slide_id: slideIds[index],
      slide_index: index,
      total_slides: slides.length,
      slide_title: title,
    };
  }

  function notifySlideChanged(force) {
    const payload = getActiveSlideInfo();
    const key = `${payload.slide_id}:${payload.slide_index}:${payload.slide_title}`;
    if (!force && key === lastSlideKey) return slideUpdateQueue;
    lastSlideKey = key;
    slideUpdateQueue = slideUpdateQueue.then(async () => {
      try {
        const response = await fetch(`${API}/api/slide/active`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(payload),
          keepalive: true,
        });
        if (!response.ok) console.warn("[SAISlideTake] Slide sync failed:", response.status);
      } catch (error) { console.warn("[SAISlideTake] Slide sync unavailable:", error); }
    });
    return slideUpdateQueue;
  }

  function send(type, payload) {
    if (socket?.readyState !== WebSocket.OPEN) return false;
    socket.send(JSON.stringify({ type, payload: payload || {} }));
    return true;
  }

  function applyParserProfileEvent(payload) {
    if (!payload?.profile || localFilePath(payload.bridge_path || "") !== localFilePath(window.location.href)) return;
    rebuildSlideIndex(payload.profile);
    lastSlideKey = "";
    notifySlideChanged(true);
  }

  function connect() {
    window.clearTimeout(reconnectTimer);
    try {
      socket = new WebSocket(SOCKET);
      socket.onopen = () => {
        send("presentation_ui_attached");
        notifySlideChanged(true);
      };
      socket.onmessage = (event) => {
        try {
          const message = JSON.parse(event.data);
          window.dispatchEvent(new CustomEvent("sai-slidetake-event", { detail: message }));
          if (message.type === "deck_parser_profile_updated") applyParserProfileEvent(message.payload || {});
          if (message.type === "start_golden_autoplay") {
            const requestedBridge = message.payload?.bridge_path;
            if (!requestedBridge || localFilePath(requestedBridge) === localFilePath(window.location.href)) {
              startGoldenAutoplay(message.payload?.start_slide_index ?? 0, message.payload?.take_slot ?? 0);
            }
          }
          if (message.type === "stop_golden_autoplay") stopGoldenAutoplay(false);
          if (message.type === "video_capture_requested") void captureSlidesForVideo(message.payload || {});
        } catch (_) { /* Ignore malformed local bridge events. */ }
      };
      socket.onerror = () => socket?.close();
      socket.onclose = () => { reconnectTimer = window.setTimeout(connect, 1200); };
    } catch (_) {
      reconnectTimer = window.setTimeout(connect, 1200);
    }
  }

  function ensureHtml2Canvas() {
    if (window.html2canvas) return Promise.resolve(window.html2canvas);
    if (html2canvasPromise) return html2canvasPromise;
    html2canvasPromise = new Promise((resolve, reject) => {
      const script = document.createElement("script");
      script.src = `${API}/sai-slidetake-html2canvas.js`;
      script.onload = () => window.html2canvas ? resolve(window.html2canvas) : reject(new Error("HTML 影像轉換器載入失敗"));
      script.onerror = () => reject(new Error("HTML 影像轉換器載入失敗"));
      (document.head || document.documentElement).appendChild(script);
    });
    return html2canvasPromise;
  }

  function localFilePath(value) {
    try {
      let path = /^[a-z]:[\\/]/i.test(value)
        ? value
        : decodeURIComponent(new URL(value, window.location.href).pathname);
      path = path.replace(/\\/g, "/");
      if (/^\/[a-z]:\//i.test(path)) path = path.slice(1);
      return path.toLowerCase().replace(/\/$/, "");
    } catch (_) { return ""; }
  }

  function cloneSlideForCapture(slide) {
    const sourceRect = slide.getBoundingClientRect();
    const width = Math.max(1, Math.round(slide.offsetWidth || sourceRect.width || window.innerWidth));
    const height = Math.max(1, Math.round(slide.offsetHeight || sourceRect.height || window.innerHeight));
    const clone = slide.cloneNode(true);
    clone.classList.remove("past", "future", "previous", "next", "bespoke-marp-past", "bespoke-marp-future");
    clone.classList.add(slide.closest(".slides") ? "present" : "active");
    clone.setAttribute("aria-hidden", "false");
    clone.style.setProperty("display", "block", "important");
    clone.style.setProperty("visibility", "visible", "important");
    clone.style.setProperty("opacity", "1", "important");
    clone.style.setProperty("position", "relative", "important");
    clone.style.setProperty("transform", "none", "important");
    clone.style.setProperty("left", "0", "important");
    clone.style.setProperty("top", "0", "important");
    clone.style.setProperty("width", `${width}px`, "important");
    clone.style.setProperty("height", `${height}px`, "important");
    let captureRoot = clone;
    for (let parent = slide.parentElement; parent && parent !== document.body && parent !== document.documentElement; parent = parent.parentElement) {
      const parentClone = parent.cloneNode(false);
      parentClone.style.setProperty("transform", "none", "important");
      parentClone.style.setProperty("visibility", "visible", "important");
      parentClone.style.setProperty("opacity", "1", "important");
      parentClone.style.setProperty("position", "relative", "important");
      parentClone.style.setProperty("width", `${width}px`, "important");
      parentClone.style.setProperty("height", `${height}px`, "important");
      parentClone.appendChild(captureRoot);
      captureRoot = parentClone;
    }
    const renderRoot = document.createElement("div");
    renderRoot.style.cssText = `position:fixed!important;left:0!important;top:0!important;width:${width}px!important;height:${height}px!important;overflow:hidden!important;opacity:0!important;z-index:-2147483647!important;pointer-events:none!important;`;
    renderRoot.setAttribute("aria-hidden", "true");
    renderRoot.setAttribute("data-sai-video-capture", "true");
    renderRoot.appendChild(captureRoot);
    (document.fullscreenElement || document.body).appendChild(renderRoot);
    return { renderRoot, captureRoot, width, height };
  }

  async function captureSlidesForVideo(payload) {
    const jobId = payload.job_id;
    const requestedIds = payload.slide_ids;
    const expectedPath = localFilePath(payload.bridge_path || "");
    if (!jobId || !Array.isArray(requestedIds) || localFilePath(window.location.href) !== expectedPath) return;
    try {
      if (requestedIds.length !== slides.length) {
        throw new Error("HTML 簡報頁數與待輸出簡報不一致");
      }
      const renderer = await ensureHtml2Canvas();
      await document.fonts?.ready;
      for (let index = 0; index < slides.length; index += 1) {
        const { renderRoot, captureRoot, width, height } = cloneSlideForCapture(slides[index]);
        try {
          const canvas = await renderer(captureRoot, {
            backgroundColor: "#000000",
            scale: Math.min(2, 3840 / width),
            useCORS: true,
            logging: false,
            width,
            height,
            windowWidth: Math.max(window.innerWidth, width),
            windowHeight: Math.max(window.innerHeight, height),
            onclone: (clonedDocument) => {
              const clonedRoot = clonedDocument.querySelector("[data-sai-video-capture]");
              if (clonedRoot) {
                clonedRoot.style.setProperty("opacity", "1", "important");
                clonedRoot.style.setProperty("visibility", "visible", "important");
              }
            },
          });
          const png = await new Promise((resolve, reject) => canvas.toBlob((blob) => blob ? resolve(blob) : reject(new Error("無法編碼投影片影像")), "image/png"));
          const response = await fetch(`${API}/api/deck/video-export/${encodeURIComponent(jobId)}/slides/${index}`, {
            method: "PUT",
            headers: { "Content-Type": "image/png" },
            body: png,
          });
          if (!response.ok) {
            const failure = await response.json().catch(() => ({}));
            throw new Error(failure.error || `第 ${index + 1} 頁影像上傳失敗`);
          }
        } finally {
          renderRoot.remove();
        }
      }
      const response = await fetch(`${API}/api/deck/video-export/${encodeURIComponent(jobId)}/finish`, { method: "POST" });
      if (!response.ok) {
        const failure = await response.json().catch(() => ({}));
        throw new Error(failure.error || "影片合成啟動失敗");
      }
    } catch (error) {
      const message = String(error);
      const response = await fetch(`${API}/api/deck/video-export/${encodeURIComponent(jobId)}/fail`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ message }),
      }).catch(() => null);
      if (!response?.ok) send("video_capture_failed", { job_id: jobId, message });
      console.warn("[SAISlideTake] Video slide capture failed:", error);
    }
  }

  function delay(milliseconds) {
    return new Promise((resolve) => window.setTimeout(resolve, milliseconds));
  }

  async function navigateToSlide(index, token) {
    if (token !== autoplayToken || index < 0 || index >= slides.length) return false;
    goToSlide(index);
    for (let attempt = 0; attempt < 30; attempt += 1) {
      if (token !== autoplayToken) return false;
      if (getActiveSlideInfo().slide_index === index) {
        await notifySlideChanged(true);
        return true;
      }
      await delay(50);
    }
    return false;
  }

  function goToSlide(index) {
    if (index < 0 || index >= slides.length) return false;
    const reveal = window.Reveal;
    if (reveal?.slide) {
      const element = slides[index];
      const root = element?.closest(".slides > section") || element;
      const roots = Array.from(document.querySelectorAll(".slides > section"));
      const horizontal = roots.indexOf(root);
      const vertical = root === element ? -1 : Array.from(root.children).filter((child) => child.tagName.toLowerCase() === "section").indexOf(element);
      reveal.slide(Math.max(0, horizontal), Math.max(0, vertical));
      return true;
    }
    const selector = document.getElementById("slide-select");
    if (selector) {
      selector.value = String(index);
      selector.dispatchEvent(new Event("change", { bubbles: true }));
    } else {
      slides.forEach((slide, slideIndex) => slide.classList.toggle("active", slideIndex === index));
    }
    return true;
  }

  function fetchManifest() {
    return fetch(`${API}/api/manifest`).then((response) => {
      if (!response.ok) throw new Error(`Manifest HTTP ${response.status}`);
      return response.json();
    });
  }

  async function playTake(group, token, takeSlot = 0) {
    const take = takeSlot > 0
      ? group?.takes?.find((item) => item.take_id === `take_${takeSlot}`)
      : group?.takes?.find((item) => item.take_id === group.golden_take_id);
    if (!take) {
      if (takeSlot > 0) return "missing";
      await delay(3000);
      return token === autoplayToken ? "missing" : "cancelled";
    }
    return new Promise((resolve, reject) => {
      autoplayAudio = new Audio(`${API}/api/audio/${encodeURIComponent(take.slide_id)}/${encodeURIComponent(take.filename)}`);
      autoplayAudio.onended = () => resolve(token === autoplayToken ? "played" : "cancelled");
      autoplayAudio.onerror = () => reject(new Error(`Audio failed for ${take.take_id}`));
      autoplayAudio.play().catch(reject);
    });
  }

  async function startGoldenAutoplay(startIndex, requestedTakeSlot = 0) {
    stopGoldenAutoplay(false);
    const token = ++autoplayToken;
    isAutoplaying = true;
    try {
      const manifest = await fetchManifest();
      const takeSlot = Number(requestedTakeSlot);
      if (!Number.isInteger(takeSlot) || takeSlot < 0 || takeSlot > 5) throw new Error("Take 欄位超出範圍");
      const first = Math.max(0, Math.min(Number(startIndex) || 0, slides.length - 1));
      const sequence = Array.from({ length: slides.length - first }, (_, offset) => first + offset)
        .filter((index) => takeSlot === 0 || Boolean(manifest.slides?.[slideIds[index]]?.takes?.some((item) => item.take_id === `take_${takeSlot}`)));
      for (let position = 0; position < sequence.length; position += 1) {
        if (token !== autoplayToken) return;
        const index = sequence[position];
        const reached = await navigateToSlide(index, token);
        if (!reached) return;
        const slide = getActiveSlideInfo();
        const group = manifest.slides?.[slide.slide_id];
        const result = await playTake(group, token, takeSlot);
        if (result === "cancelled" || token !== autoplayToken) return;
        if (result === "played" && position < sequence.length - 1) await delay(manifest.transition_delay_ms ?? 400);
      }
      isAutoplaying = false;
      send("autoplay_finished");
    } catch (error) {
      isAutoplaying = false;
      send("autoplay_error", { message: String(error) });
      console.warn("[SAISlideTake] Golden autoplay stopped:", error);
    } finally {
      if (token === autoplayToken) autoplayAudio = null;
    }
  }

  function stopGoldenAutoplay(notifyServer) {
    autoplayToken += 1;
    isAutoplaying = false;
    if (autoplayAudio) {
      autoplayAudio.pause();
      autoplayAudio.src = "";
      autoplayAudio = null;
    }
    if (notifyServer) send("stop_golden_autoplay");
  }

  stateObserver = new MutationObserver(() => notifySlideChanged(false));
  watchActiveState();

  window.addEventListener("keydown", (event) => {
    if (event.repeat || event.target?.matches?.("input, textarea, select, [contenteditable='true']")) return;
    const key = event.key.toLowerCase();
    if (key === "r" || event.code === "Space") {
      event.preventDefault();
      event.stopImmediatePropagation();
      send("keyboard_shortcut", { key: key === "r" ? "r" : "space" });
    }
  }, true);

  window.addEventListener("beforeunload", () => {
    stateObserver.disconnect();
    stopGoldenAutoplay(false);
    socket?.close();
  });

  connect();
  notifySlideChanged(true);
  window.SAISlideTakeBridge = {
    getActiveSlideInfo,
    notifySlideChanged: () => notifySlideChanged(true),
    send,
    startGoldenAutoplay,
    stopGoldenAutoplay: () => stopGoldenAutoplay(true),
  };

  const embeddedUi = document.createElement("script");
  embeddedUi.src = `${API}/sai-slidetake-embedded.js`;
  embeddedUi.async = true;
  embeddedUi.onerror = () => console.warn("[SAISlideTake] Embedded presentation UI failed to load.");
  (document.head || document.documentElement).appendChild(embeddedUi);
})();
