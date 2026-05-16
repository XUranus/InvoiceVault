import React from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X, Copy } from "lucide-react";

const appIcon = new URL("../../src-tauri/icons/icon.png", import.meta.url).href;

export function TitleBar() {
  const appWindow = React.useMemo(() => getCurrentWindow(), []);
  const [isMaximized, setIsMaximized] = React.useState(false);

  React.useEffect(() => {
    let unlisten: (() => void) | null = null;
    appWindow.isMaximized().then(setIsMaximized);
    appWindow
      .onResized(() => {
        appWindow.isMaximized().then(setIsMaximized);
      })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});
    return () => {
      unlisten?.();
    };
  }, [appWindow]);

  return (
    <div className="titlebar" data-tauri-drag-region>
      <div className="titlebar-brand" data-tauri-drag-region>
        <img src={appIcon} className="titlebar-icon" alt="" aria-hidden="true" />
        <span className="titlebar-title">InvoiceVault</span>
      </div>
      <div className="titlebar-controls">
        <button
          className="titlebar-btn"
          onClick={() => appWindow.minimize()}
          title="最小化"
        >
          <Minus size={16} />
        </button>
        <button
          className="titlebar-btn"
          onClick={() => appWindow.toggleMaximize()}
          title={isMaximized ? "还原" : "最大化"}
        >
          {isMaximized ? <Copy size={14} /> : <Square size={14} />}
        </button>
        <button
          className="titlebar-btn titlebar-btn-close"
          onClick={() => appWindow.close()}
          title="关闭"
        >
          <X size={16} />
        </button>
      </div>
    </div>
  );
}

export default TitleBar;
