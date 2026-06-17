import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import React, { useEffect, useMemo, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import "./styles.css";

type Mode = "attached" | "detached";

type DesktopInputEvent = {
  kind: "click";
  x: number;
  y: number;
  width: number;
  height: number;
};

type AttachDiagnostics = {
  progmanFound: boolean;
  standardWorkerFound: boolean;
  progmanWorkerFound: boolean;
  workerFound: boolean;
  attached: boolean;
  visible: boolean;
  parentIsWorkerW: boolean;
  hwnd: number;
  parent: number;
  workerW: number;
  candidateCount: number;
  className: string;
  title: string;
  styleHex: string;
  exStyleHex: string;
  windowRect: RectDiagnostics;
  clientRect: RectDiagnostics;
  foreground: number;
  isForeground: boolean;
  nativeHookInstalled: boolean;
  nativeMsg: string;
  phase: string;
  error: string | null;
};

type RectDiagnostics = {
  left: number;
  top: number;
  right: number;
  bottom: number;
  width: number;
  height: number;
};

const emptyRect: RectDiagnostics = {
  left: 0,
  top: 0,
  right: 0,
  bottom: 0,
  width: 0,
  height: 0
};

const emptyDiagnostics: AttachDiagnostics = {
  progmanFound: false,
  standardWorkerFound: false,
  progmanWorkerFound: false,
  workerFound: false,
  attached: false,
  visible: false,
  parentIsWorkerW: false,
  hwnd: 0,
  parent: 0,
  workerW: 0,
  candidateCount: 0,
  className: "",
  title: "",
  styleHex: "",
  exStyleHex: "",
  windowRect: emptyRect,
  clientRect: emptyRect,
  foreground: 0,
  isForeground: false,
  nativeHookInstalled: false,
  nativeMsg: "",
  phase: "",
  error: null
};

function App() {
  const appWindow = useMemo(() => getCurrentWindow(), []);
  const switchButtonRef = useRef<HTMLButtonElement | null>(null);
  const panelRef = useRef<HTMLElement | null>(null);
  const debugSeqRef = useRef(0);
  const modeRef = useRef<Mode>("attached");
  const diagnosticsRef = useRef<AttachDiagnostics>(emptyDiagnostics);
  const scaleFactorRef = useRef(1);
  const closingRef = useRef(false);
  const finishScheduledRef = useRef(false);

  const [mode, setMode] = useState<Mode>("attached");
  const [clicks, setClicks] = useState(0);
  const [lastEvent, setLastEvent] = useState("started");
  const [point, setPoint] = useState({ x: 0, y: 0 });
  const [diagnostics, setDiagnostics] = useState<AttachDiagnostics>(emptyDiagnostics);
  const [debugLines, setDebugLines] = useState<string[]>([]);

  const appendDebug = (label: string, data?: Partial<AttachDiagnostics>) => {
    const seq = ++debugSeqRef.current;
    const source = data ?? diagnosticsRef.current;
    const currentMode = modeRef.current;
    const line = [
      `${seq}. ${label}`,
      `mode=${currentMode}`,
      `phase=${source.phase ?? ""}`,
      `hwnd=${hex(source.hwnd)}`,
      `parent=${hex(source.parent)}`,
      `worker=${hex(source.workerW)}`,
      `attached=${yesNo(Boolean(source.attached))}`,
      `visible=${yesNo(Boolean(source.visible))}`,
      `style=${source.styleHex ?? ""}`,
      `ex=${source.exStyleHex ?? ""}`,
      `rect=${rectText(source.windowRect)}`,
      `client=${rectText(source.clientRect)}`,
      `fg=${yesNo(Boolean(source.isForeground))}`,
      `hook=${yesNo(Boolean(source.nativeHookInstalled))}`,
      `native=${source.nativeMsg ?? ""}`,
      `n=${source.nativeCount ?? 0}`,
      `title="${source.title ?? ""}"`,
      `class=${source.className ?? ""}`
    ].join(" | ");

    setDebugLines((current) => [line, ...current].slice(0, 18));
  };

  const refreshDiagnostics = async () => {
    const next = await invoke<AttachDiagnostics>("get_attach_diagnostics");
    setDiagnostics(next);
    setMode(next.attached ? "attached" : "detached");
    diagnosticsRef.current = next;
    modeRef.current = next.attached ? "attached" : "detached";
    appendDebug("manual diag", next);
    return next;
  };

  useEffect(() => {
    void appWindow.scaleFactor().then((factor) => {
      scaleFactorRef.current = factor || 1;
    });

    void refreshDiagnostics();

    const inputUnlistenPromise = listen<DesktopInputEvent>("desktop-input", (event) => {
      const payload = event.payload;
      setPoint({ x: payload.x, y: payload.y });

      if (payload.kind === "click") {
        setClicks((current) => current + 1);
        setLastEvent("click");
        appendDebug(`attached click x=${payload.x} y=${payload.y}`);

        const cssX = payload.x / scaleFactorRef.current;
        const cssY = payload.y / scaleFactorRef.current;
        const buttonRect = switchButtonRef.current?.getBoundingClientRect();

        if (
          buttonRect &&
          cssX >= buttonRect.left - 8 &&
          cssX <= buttonRect.right + 8 &&
          cssY >= buttonRect.top - 8 &&
          cssY <= buttonRect.bottom + 8
        ) {
          void switchMode("detached");
        }
      }
    });
    const closeUnlistenPromise = listen("close-prepared", () => {
      setMode("detached");
      modeRef.current = "detached";
      setLastEvent("closing");
      appendDebug("close-prepared");
      scheduleFinishClose(1200);
    });
    const debugUnlistenPromise = listen<AttachDiagnostics>("debug-snapshot", (event) => {
      setDiagnostics(event.payload);
      appendDebug("backend", event.payload);
    });

    return () => {
      void inputUnlistenPromise.then((unlisten) => unlisten());
      void closeUnlistenPromise.then((unlisten) => unlisten());
      void debugUnlistenPromise.then((unlisten) => unlisten());
    };
  }, [appWindow]);

  const switchMode = async (target?: Mode) => {
    const nextMode = target ?? (mode === "attached" ? "detached" : "attached");
    appendDebug(`switch request ${mode}->${nextMode}`);
    if (nextMode === "attached") {
      try {
        const result = await invoke<AttachDiagnostics>("switch_to_attached");
        setDiagnostics(result);
        diagnosticsRef.current = result;
        modeRef.current = result.attached ? "attached" : "detached";
        setMode(result.attached ? "attached" : "detached");
        setLastEvent(result.attached ? "attached" : "attach failed");
        appendDebug("switch attached result", result);
      } catch (error) {
        setLastEvent(`attach failed: ${String(error)}`);
        appendDebug(`switch attached error ${String(error)}`);
        void refreshDiagnostics();
      }
      return;
    }

    await invoke("switch_to_detached");
    setMode("detached");
    modeRef.current = "detached";
    setLastEvent("detached");
    appendDebug("switch detached result");
    void refreshDiagnostics();
  };

  const closeApp = async () => {
    if (closingRef.current) {
      return;
    }

    closingRef.current = true;
    setLastEvent("closing");
    modeRef.current = "detached";
    appendDebug("close request");
    await invoke("prepare_close_app");
    setMode("detached");
    scheduleFinishClose(1200);
  };

  const scheduleFinishClose = (delay: number) => {
    if (finishScheduledRef.current) {
      return;
    }

    finishScheduledRef.current = true;
    window.setTimeout(() => {
      void invoke("finish_close_app");
    }, delay);
  };

  const handlePointerMove = (event: React.PointerEvent<HTMLElement>) => {
    if (mode !== "detached") {
      return;
    }

    const rect = panelRef.current?.getBoundingClientRect();
    setPoint({
      x: Math.round(event.clientX - (rect?.left ?? 0)),
      y: Math.round(event.clientY - (rect?.top ?? 0))
    });
  };

  const handleClick = (event: React.MouseEvent<HTMLElement>) => {
    if (mode !== "detached") {
      return;
    }

    const rect = panelRef.current?.getBoundingClientRect();
    setClicks((current) => current + 1);
    setLastEvent("click");
    appendDebug(`detached click x=${Math.round(event.clientX)} y=${Math.round(event.clientY)}`);
    setPoint({
      x: Math.round(event.clientX - (rect?.left ?? 0)),
      y: Math.round(event.clientY - (rect?.top ?? 0))
    });
  };

  const startDrag = async (event: React.PointerEvent<HTMLDivElement>) => {
    if (mode === "detached" && event.button === 0) {
      appendDebug("drag start");
      await appWindow.startDragging();
    }
  };

  const startResize = async (event: React.PointerEvent<HTMLDivElement>) => {
    if (mode === "detached" && event.button === 0) {
      event.preventDefault();
      event.stopPropagation();
      appendDebug("resize start");
      await appWindow.startResizeDragging("SouthEast");
    }
  };

  return (
    <main
      ref={panelRef}
      className={`widget widget-${mode}`}
      onPointerMove={handlePointerMove}
      onClick={handleClick}
    >
      <section className="header">
        <div>
          <span className="mode-pill">{mode}</span>
          <h1>Wallpaper Widget Test</h1>
        </div>
        <div
          className={`drag-zone ${mode === "detached" ? "is-active" : ""}`}
          onPointerDown={startDrag}
          aria-hidden="true"
        />
        <div className="actions">
          <button ref={switchButtonRef} className="primary" onClick={() => void switchMode()}>
            {mode === "attached" ? "Detach" : "Attach"}
          </button>
          <button onClick={() => void refreshDiagnostics()}>Diag</button>
          <button className="danger" onClick={closeApp}>
            Close
          </button>
        </div>
      </section>

      <section className="status-grid">
        <div className="status-item">
          <span>Clicks</span>
          <strong>{clicks}</strong>
        </div>
        <div className="status-item">
          <span>Mouse</span>
          <strong>
            {point.x}, {point.y}
          </strong>
        </div>
        <div className="status-item">
          <span>Event</span>
          <strong>{lastEvent}</strong>
        </div>
      </section>

      <section className="diagnostics">
        <span>Progman {yesNo(diagnostics.progmanFound)}</span>
        <span>StdWorker {yesNo(diagnostics.standardWorkerFound)}</span>
        <span>ProgWorker {yesNo(diagnostics.progmanWorkerFound)}</span>
        <span>ParentOK {yesNo(diagnostics.parentIsWorkerW)}</span>
        <span>Visible {yesNo(diagnostics.visible)}</span>
        <span>Hosts {diagnostics.candidateCount}</span>
        <span>Worker 0x{diagnostics.workerW.toString(16)}</span>
        <span>HWND {hex(diagnostics.hwnd)}</span>
        <span>Parent {hex(diagnostics.parent)}</span>
        <span>Style {diagnostics.styleHex}</span>
        <span>Ex {diagnostics.exStyleHex}</span>
        <span>Rect {rectText(diagnostics.windowRect)}</span>
        <span>Client {rectText(diagnostics.clientRect)}</span>
        <span>FG {yesNo(diagnostics.isForeground)}</span>
        <span>Hook {yesNo(diagnostics.nativeHookInstalled)}</span>
        <span>Native {diagnostics.nativeMsg}</span>
        <span>Count {diagnostics.nativeCount}</span>
        <span>Title "{diagnostics.title}"</span>
        <span className="diagnostic-error">{diagnostics.error ?? ""}</span>
      </section>

      <section className="debug-log">
        {debugLines.map((line, index) => (
          <div key={`${index}-${line}`}>{line}</div>
        ))}
      </section>

      <p>
        attached is parented to WorkerW below desktop icons. detached is a normal
        frameless Tauri window. Both modes support click counting.
      </p>

      {mode === "detached" && (
        <div className="resize-corner" onPointerDown={startResize} title="Resize" />
      )}
    </main>
  );
}

function yesNo(value: boolean) {
  return value ? "Y" : "N";
}

function hex(value?: number) {
  if (!value) {
    return "0x0";
  }
  return `0x${Math.trunc(value).toString(16)}`;
}

function rectText(rect?: AttachDiagnostics["windowRect"]) {
  if (!rect) {
    return "n/a";
  }
  return `${rect.left},${rect.top},${rect.width}x${rect.height}`;
}

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
