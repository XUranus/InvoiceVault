import { useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import type { WatcherImportEvent } from "../types";
import {
  importFiles,
  getRecognitionQueueStatus,
  syncAllEmailSources,
  listEmailSources,
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

export function useAppInitializer() {
  const navigate = useNavigate();

  const theme = useAppStore((s) => s.theme);
  const setTheme = useAppStore((s) => s.setTheme);
  const initialize = useAppStore((s) => s.initialize);
  const loadConfigFromBackend = useLlmStore((s) => s.loadConfigFromBackend);
  const setIsDraggingFiles = useAppStore((s) => s.setIsDraggingFiles);
  const setError = useAppStore((s) => s.setError);
  const setImportBadgeCount = useAppStore((s) => s.setImportBadgeCount);
  const refreshInvoices = useAppStore((s) => s.refreshInvoices);
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
      .then((fn) => {
        cleanup = fn;
      })
      .catch(() => {});
    return () => {
      cleanup?.();
    };
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

  // Global drag-drop handler
  // Uses module-level guard to survive React StrictMode double-mount
  useEffect(() => {
    if (dragDropRegistered) return;
    dragDropRegistered = true;

    getCurrentWebview()
      .onDragDropEvent((event) => {
        const evtType = (event.payload as { type: string }).type;

        if (evtType === "enter" || evtType === "over") {
          setIsDraggingFiles(true);
          return;
        }

        if (evtType === "leave") {
          setIsDraggingFiles(false);
          return;
        }

        setIsDraggingFiles(false);
        const paths: string[] = (event.payload as { paths: string[] }).paths ?? [];
        if (paths.length === 0) return;

        // Guard against duplicate drop events from Tauri on Linux/GTK
        if (dropImportInProgress) return;
        dropImportInProgress = true;

        importFiles(paths)
          .then(() => {
            triggerImportRefresh();
            navigate("/import");
          })
          .catch((err) => setError(String(err)))
          .finally(() => { dropImportInProgress = false; });
      })
      .catch(() => {});
  }, [
    setIsDraggingFiles,
    setError,
    triggerImportRefresh,
    navigate,
  ]);

  // Native window drag-drop fallback. On Windows, WebView-level file-drop
  // events can be swallowed before the frontend handler sees them.
  useEffect(() => {
    let cleanupDragState: (() => void) | null = null;
    let cleanupImportError: (() => void) | null = null;

    listen<{ dragging: boolean }>("native-drag-state", (event) => {
      setIsDraggingFiles(event.payload.dragging);
    })
      .then((fn) => {
        cleanupDragState = fn;
      })
      .catch(() => {});

    listen<string>("native-import-error", (event) => {
      setError(event.payload);
    })
      .then((fn) => {
        cleanupImportError = fn;
      })
      .catch(() => {});

    return () => {
      cleanupDragState?.();
      cleanupImportError?.();
    };
  }, [setIsDraggingFiles, setError]);

  // Watcher auto-import listener
  useEffect(() => {
    let cleanup: (() => void) | null = null;
    listen<WatcherImportEvent>(
      "watcher-import",
      (_event) => {
        triggerImportRefresh();
        refreshInvoices();
        triggerDashboardRefresh();
      },
    )
      .then((fn) => {
        cleanup = fn;
      })
      .catch(() => {});
    return () => {
      cleanup?.();
    };
  }, [triggerImportRefresh, refreshInvoices, triggerDashboardRefresh]);

  // Background recognition completion listener
  useEffect(() => {
    let cleanup: (() => void) | null = null;
    listen("recognition-complete", () => {
      refreshInvoices();
      triggerImportRefresh();
      triggerDashboardRefresh();
    })
      .then((fn) => {
        cleanup = fn;
      })
      .catch(() => {});
    return () => {
      cleanup?.();
    };
  }, [refreshInvoices, triggerImportRefresh, triggerDashboardRefresh]);

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

  // Email sync polling (per-source configurable interval)
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | null = null;
    let stopped = false;

    const tick = async () => {
      if (stopped) return;
      try {
        // Determine the minimum poll interval from enabled sources
        const sources = await listEmailSources();
        const enabledSources = sources.filter((s) => s.enabled);
        if (enabledSources.length > 0) {
          const results = await syncAllEmailSources();
          const totalImported = results.reduce(
            (sum, r) => sum + r.imported_count,
            0,
          );
          if (totalImported > 0) {
            triggerImportRefresh();
            refreshInvoices();
            triggerDashboardRefresh();
          }
        }
        // Use the minimum interval among enabled sources, default 60s
        const minInterval = enabledSources.length > 0
          ? Math.min(...enabledSources.map((s) => s.poll_interval_seconds || 60))
          : 60;
        if (!stopped) {
          timer = setTimeout(tick, Math.max(minInterval, 10) * 1000);
        }
      } catch {
        if (!stopped) {
          timer = setTimeout(tick, 60000);
        }
      }
    };

    // Start after a short delay to let the app initialize
    timer = setTimeout(tick, 5000);

    return () => {
      stopped = true;
      if (timer) clearTimeout(timer);
    };
  }, [triggerImportRefresh, refreshInvoices, triggerDashboardRefresh]);
}
