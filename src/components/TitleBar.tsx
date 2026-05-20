import React from "react";
import { invoke } from "@tauri-apps/api/core";
import { Copy, Minus, Square, X } from "lucide-react";

const appIcon = new URL("../../src-tauri/icons/icon.png", import.meta.url).href;

export function TitleBar() {
  const [isMaximized, setIsMaximized] = React.useState(false);

  const startDrag = React.useCallback(
    (event: React.MouseEvent<HTMLDivElement>) => {
      if (event.button !== 0) return;
      invoke("window_start_dragging").catch(() => {});
    },
    [],
  );

  const toggleMaximize = React.useCallback(() => {
    invoke<boolean>("window_toggle_maximize")
      .then(setIsMaximized)
      .catch(() => {});
  }, []);

  return (
    <div className="titlebar">
      <div
        className="titlebar-drag-region"
        onMouseDown={startDrag}
        onDoubleClick={toggleMaximize}
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
