import { useCallback, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import type { WatcherImportEvent } from "../types";
import {
  importFiles,
  getRecognitionQueueStatus,
  syncAllEmailSources,
  listEmailSources,
  frontendHeartbeat,
} from "../api";
import { useAppStore } from "../stores/appStore";
import { useLlmStore } from "../stores/llmStore";
import { useRefreshStore } from "../stores/refreshStore";

// Module-level guard: prevents duplicate drag-drop handler registration
// caused by React StrictMode double-mount (mount → unmount → mount)
let dragDropRegistered = false;

// Mutex guard: Tauri's onDragDropEvent fires multiple "drop" events
// for a single physical file drop on Linux/GTK. A boolean lock ensures
// only the first event triggers import; the rest are dropped immediately.
let dropImportInProgress = false;

// Debounce timer for coalescing rapid refresh triggers
// (watcher-import, recognition-complete, and email-sync can fire within
// hundreds of ms of each other, causing redundant 10+ IPC call bursts)
let refreshTimer: ReturnType<typeof setTimeout> | null = null;
function scheduleRefresh(
  triggerImport: () => void,
  triggerDashboard: () => void,
  refreshBadge: () => void,
) {
  if (refreshTimer) clearTimeout(refreshTimer);
  refreshTimer = setTimeout(() => {
    refreshTimer = null;
    triggerImport();
    refreshBadge();
    triggerDashboard();
  }, 300);
}

export function useAppInitializer() {
  const navigate = useNavigate();

  const theme = useAppStore((s) => s.theme);
  const setTheme = useAppStore((s) => s.setTheme);
  const initialize = useAppStore((s) => s.initialize);
  const loadConfigFromBackend = useLlmStore((s) => s.loadConfigFromBackend);
  const setIsDraggingFiles = useAppStore((s) => s.setIsDraggingFiles);
  const setDragImportPaths = useAppStore((s) => s.setDragImportPaths);
  const clearDragImportPaths = useAppStore((s) => s.clearDragImportPaths);
  const setError = useAppStore((s) => s.setError);
  const setImportBadgeCount = useAppStore((s) => s.setImportBadgeCount);
  const refreshUnviewedCount = useAppStore((s) => s.refreshUnviewedCount);
  const triggerImportRefresh = useRefreshStore(
    (s) => s.triggerImportRefresh,
  );
  const triggerDashboardRefresh = useRefreshStore(
    (s) => s.triggerDashboardRefresh,
  );

  // Theme DOM sync
  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
  }, [theme]);

  // Listen for theme changes from Agent/backend
  useEffect(() => {
    let cleanup: (() => void) | null = null;
    listen<{ theme: string }>("theme-change", (event) => {
      const t = event.payload.theme;
      if (t === "light" || t === "dark") {
        setTheme(t);
      }
    })
      .then((fn) => { cleanup = fn; })
      .catch(() => {});
    return () => { cleanup?.(); };
  }, [setTheme]);

  // Initialize data on mount
  useEffect(() => {
    initialize();
    loadConfigFromBackend().then(() => {
      const { llm } = useLlmStore.getState();
      const dismissed = localStorage.getItem("onboarding_dismissed") === "1";
      if (!llm.config.apiKey.trim() && !dismissed) {
        useAppStore.getState().setShowOnboarding(true);
      }
    }).catch(() => {});
  }, [initialize, loadConfigFromBackend]);

  const handleDroppedFiles = useCallback(
    (paths: string[]) => {
      if (paths.length === 0) return;
      if (dropImportInProgress) return;
      dropImportInProgress = true;
      setIsDraggingFiles(false);
      setDragImportPaths(paths);
      navigate("/import");
      importFiles(paths)
        .then(() => {
          triggerImportRefresh();
        })
        .catch((err) => setError(String(err)))
        .finally(() => {
          clearDragImportPaths();
          dropImportInProgress = false;
        });
    },
    [
      clearDragImportPaths,
      navigate,
      setDragImportPaths,
      setError,
      setIsDraggingFiles,
      triggerImportRefresh,
    ],
  );

  // Global drag-drop handler
  useEffect(() => {
    if (dragDropRegistered) return;
    dragDropRegistered = true;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        const evtType = (event.payload as { type: string }).type;
        if (evtType === "enter" || evtType === "over") {
          setIsDraggingFiles(true); return;
        }
        if (evtType === "leave") {
          setIsDraggingFiles(false); return;
        }
        setIsDraggingFiles(false);
        const paths: string[] = (event.payload as { paths: string[] }).paths ?? [];
        handleDroppedFiles(paths);
      })
      .catch(() => {});
  }, [setIsDraggingFiles, handleDroppedFiles]);

  // Native window drag-drop fallback
  useEffect(() => {
    let cleanupDragState: (() => void) | null = null;
    let cleanupDrop: (() => void) | null = null;
    let cleanupImportError: (() => void) | null = null;
    listen<{ dragging: boolean }>("native-drag-state", (event) => {
      setIsDraggingFiles(event.payload.dragging);
    }).then((fn) => { cleanupDragState = fn; }).catch(() => {});
    listen<{ paths: string[] }>("native-file-drop", (event) => {
      handleDroppedFiles(event.payload.paths);
    }).then((fn) => { cleanupDrop = fn; }).catch(() => {});
    listen<string>("native-import-error", (event) => {
      setError(event.payload);
    }).then((fn) => { cleanupImportError = fn; }).catch(() => {});
    return () => {
      cleanupDragState?.();
      cleanupDrop?.();
      cleanupImportError?.();
    };
  }, [setIsDraggingFiles, setError, handleDroppedFiles]);

  // Watcher auto-import listener
  useEffect(() => {
    let cleanup: (() => void) | null = null;
    listen<WatcherImportEvent>("watcher-import", () => {
      scheduleRefresh(triggerImportRefresh, triggerDashboardRefresh, refreshUnviewedCount);
    }).then((fn) => { cleanup = fn; }).catch(() => {});
    return () => { cleanup?.(); };
  }, [triggerImportRefresh, triggerDashboardRefresh, refreshUnviewedCount]);

  // Background recognition completion listener
  useEffect(() => {
    let cleanup: (() => void) | null = null;
    listen("recognition-complete", () => {
      scheduleRefresh(triggerImportRefresh, triggerDashboardRefresh, refreshUnviewedCount);
    }).then((fn) => { cleanup = fn; }).catch(() => {});
    return () => { cleanup?.(); };
  }, [triggerImportRefresh, triggerDashboardRefresh, refreshUnviewedCount]);

  // Recognition queue polling
  useEffect(() => {
    const poll = () => {
      getRecognitionQueueStatus()
        .then((status) => {
          const total = status.pending + status.running;
          setImportBadgeCount(total);
        })
        .catch(() => {});
    };
    poll();
    const interval = setInterval(poll, 2000);
    return () => clearInterval(interval);
  }, [setImportBadgeCount]);

  // Email sync polling
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | null = null;
    let stopped = false;
    const tick = async () => {
      if (stopped) return;
      try {
        const sources = await listEmailSources();
        const hasAutoSync = sources.some(
          (s) => s.enabled,
        );
        if (!hasAutoSync) return;
        await syncAllEmailSources();
        scheduleRefresh(triggerImportRefresh, triggerDashboardRefresh, refreshUnviewedCount);
      } catch {
        // ignore sync errors
      } finally {
        if (!stopped) {
          timer = setTimeout(tick, 60_000);
        }
      }
    };
    timer = setTimeout(tick, 5000);
    return () => { stopped = true; if (timer) clearTimeout(timer); };
  }, [triggerImportRefresh, triggerDashboardRefresh, refreshUnviewedCount]);

  // Frontend heartbeat
  useEffect(() => {
    let seq = 0;
    const interval = setInterval(() => {
      seq++;
      frontendHeartbeat(seq).catch(() => {});
    }, 5000);
    return () => clearInterval(interval);
  }, []);
}
