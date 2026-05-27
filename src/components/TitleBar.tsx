import React from "react";
import { invoke } from "@tauri-apps/api/core";
import { Copy, Minus, Square, X } from "lucide-react";

const appIcon = new URL("../../src-tauri/icons/icon.png", import.meta.url).href;

export function TitleBar() {
  const [isMaximized, setIsMaximized] = React.useState(false);
  const lastClickRef = React.useRef<number>(0);
  const draggingRef = React.useRef(false);
  const dragOriginRef = React.useRef({ x: 0, y: 0 });
  const winPosRef = React.useRef({ x: 0, y: 0 });

  const toggleMaximize = React.useCallback(() => {
    invoke<boolean>("window_toggle_maximize")
      .then(setIsMaximized)
      .catch(() => {});
  }, []);

  React.useEffect(() => {
    const onMove = (e: MouseEvent) => {
      if (!draggingRef.current) return;
      const dx = e.screenX - dragOriginRef.current.x;
      const dy = e.screenY - dragOriginRef.current.y;
      invoke("window_set_position", {
        x: winPosRef.current.x + dx,
        y: winPosRef.current.y + dy,
      }).catch(() => {});
    };

    const onUp = () => {
      draggingRef.current = false;
    };

    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, []);

  const handleMouseDown = React.useCallback(
    async (event: React.MouseEvent<HTMLDivElement>) => {
      if (event.button !== 0) return;
      const now = Date.now();

      // Double-click detected — toggle maximize instead of dragging
      if (now - lastClickRef.current < 300) {
        lastClickRef.current = 0;
        toggleMaximize();
        return;
      }
      lastClickRef.current = now;

      // Start manual drag: record origin and window position
      const mouseX = event.screenX;
      const mouseY = event.screenY;
      try {
        const pos = await invoke<{ x: number; y: number }>("window_get_position");
        winPosRef.current = pos;
        dragOriginRef.current = { x: mouseX, y: mouseY };
        draggingRef.current = true;
      } catch {
        // fallback to native drag
        invoke("window_start_dragging").catch(() => {});
      }
    },
    [toggleMaximize],
  );

  return (
    <div className="titlebar">
      <div
        className="titlebar-drag-region"
        onMouseDown={handleMouseDown}
      >
        <div className="titlebar-brand">
          <img src={appIcon} className="titlebar-icon" alt="" aria-hidden="true" />
          <span className="titlebar-title">InvoiceVault</span>
        </div>
      </div>
      <div className="titlebar-controls">
        <button
          className="titlebar-btn"
          onMouseDown={(event) => event.stopPropagation()}
          onClick={() => invoke("window_minimize").catch(() => {})}
          title="Minimize"
        >
          <Minus size={16} />
        </button>
        <button
          className="titlebar-btn"
          onMouseDown={(event) => event.stopPropagation()}
          onClick={toggleMaximize}
          title={isMaximized ? "Restore" : "Maximize"}
        >
          {isMaximized ? <Copy size={14} /> : <Square size={14} />}
        </button>
        <button
          className="titlebar-btn titlebar-btn-close"
          onMouseDown={(event) => event.stopPropagation()}
          onClick={() => invoke("window_close").catch(() => {})}
          title="Close"
        >
          <X size={16} />
        </button>
      </div>
    </div>
  );
}

export default TitleBar;
