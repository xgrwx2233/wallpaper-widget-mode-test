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
  error: string | null;
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
  error: null
};

function App() {
  const appWindow = useMemo(() => getCurrentWindow(), []);
  const switchButtonRef = useRef<HTMLButtonElement | null>(null);
  const panelRef = useRef<HTMLElement | null>(null);
  const scaleFactorRef = useRef(1);

  const [mode, setMode] = useState<Mode>("attached");
  const [hovered, setHovered] = useState(false);
  const [clicks, setClicks] = useState(0);
  const [lastEvent, setLastEvent] = useState("started");
  const [point, setPoint] = useState({ x: 0, y: 0 });
  const [diagnostics, setDiagnostics] = useState<AttachDiagnostics>(emptyDiagnostics);

  const refreshDiagnostics = async () => {
    const next = await invoke<AttachDiagnostics>("get_attach_diagnostics");
    setDiagnostics(next);
    setMode(next.attached ? "attached" : "detached");
    return next;
  };

  useEffect(() => {
    void appWindow.scaleFactor().then((factor) => {
      scaleFactorRef.current = factor || 1;
    });

    void refreshDiagnostics();

    const unlistenPromise = listen<DesktopInputEvent>("desktop-input", (event) => {
      const payload = event.payload;
      setPoint({ x: payload.x, y: payload.y });

      if (payload.kind === "click") {
        setClicks((current) => current + 1);
        setHovered(true);
        setLastEvent("click");

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

    return () => {
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [appWindow]);

  const switchMode = async (target?: Mode) => {
    const nextMode = target ?? (mode === "attached" ? "detached" : "attached");
    if (nextMode === "attached") {
      try {
        const result = await invoke<AttachDiagnostics>("switch_to_attached");
        setDiagnostics(result);
        setMode(result.attached ? "attached" : "detached");
        setLastEvent(result.attached ? "attached" : "attach failed");
      } catch (error) {
        setLastEvent(`attach failed: ${String(error)}`);
        void refreshDiagnostics();
      }
      return;
    }

    await invoke("switch_to_detached");
    setMode("detached");
    setHovered(false);
    setLastEvent("detached");
    void refreshDiagnostics();
  };

  const closeApp = async () => {
    await invoke("close_app");
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

  const handlePointerLeave = () => {
    if (mode !== "detached") {
      return;
    }

    setHovered(false);
  };

  const handleClick = (event: React.MouseEvent<HTMLElement>) => {
    if (mode !== "detached") {
      return;
    }

    const rect = panelRef.current?.getBoundingClientRect();
    setClicks((current) => current + 1);
    setHovered(true);
    setLastEvent("click");
    setPoint({
      x: Math.round(event.clientX - (rect?.left ?? 0)),
      y: Math.round(event.clientY - (rect?.top ?? 0))
    });
  };

  const startDrag = async (event: React.PointerEvent<HTMLDivElement>) => {
    if (mode === "detached" && event.button === 0) {
      await appWindow.startDragging();
    }
  };

  const startResize = async (event: React.PointerEvent<HTMLDivElement>) => {
    if (mode === "detached" && event.button === 0) {
      event.preventDefault();
      event.stopPropagation();
      await appWindow.startResizeDragging("SouthEast");
    }
  };

  return (
    <main
      ref={panelRef}
      className={`widget widget-${mode} ${hovered ? "is-hovered" : ""}`}
      onPointerMove={handlePointerMove}
      onPointerLeave={handlePointerLeave}
      onClick={handleClick}
    >
      {mode === "detached" && (
        <div className="drag-strip" onPointerDown={startDrag}>
          Drag window
        </div>
      )}

      <section className="header">
        <div>
          <span className="mode-pill">{mode}</span>
          <h1>Wallpaper Widget Test</h1>
        </div>
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

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
