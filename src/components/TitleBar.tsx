import React from "react";
import { invoke } from "@tauri-apps/api/core";
import { Copy, Minus, Square, X } from "lucide-react";

const appIcon = new URL("../../src-tauri/icons/icon.png", import.meta.url).href;

export function TitleBar() {
  const [isMaximized, setIsMaximized] = React.useState(false);
  const lastClickRef = React.useRef<number>(0);
  const dragTimerRef = React.useRef<ReturnType<typeof setTimeout> | null>(null);
  const buttonHeldRef = React.useRef(false);

  const toggleMaximize = React.useCallback(() => {
    invoke<boolean>("window_toggle_maximize")
      .then(setIsMaximized)
      .catch(() => {});
  }, []);

  const handleMouseDown = React.useCallback(
    (event: React.MouseEvent<HTMLDivElement>) => {
      if (event.button !== 0) return;
      const now = Date.now();

      // Double-click — toggle maximize
      if (now - lastClickRef.current < 300) {
        lastClickRef.current = 0;
        if (dragTimerRef.current) {
          clearTimeout(dragTimerRef.current);
          dragTimerRef.current = null;
        }
        toggleMaximize();
        return;
      }
      lastClickRef.current = now;

      // Defer native drag by 350ms (exceeds 300ms double-click window).
      // If the user releases the mouse before the timer fires, the mouseup
      // listener cancels the timer so the drag never starts.
      buttonHeldRef.current = true;

      dragTimerRef.current = setTimeout(() => {
        dragTimerRef.current = null;
        if (buttonHeldRef.current) {
          invoke("window_start_dragging").catch(() => {});
        }
      }, 350);

      // Cancel pending drag if the mouse is released before the timer fires
      const onUp = () => {
        buttonHeldRef.current = false;
        if (dragTimerRef.current) {
          clearTimeout(dragTimerRef.current);
          dragTimerRef.current = null;
        }
        window.removeEventListener("mouseup", onUp);
      };
      window.addEventListener("mouseup", onUp);
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
          <span className="titlebar-title">票匣</span>
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
