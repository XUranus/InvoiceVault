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

export function useAppInitializer() {
  const navigate = useNavigate();

  const theme = useAppStore((s) => s.theme);
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

  // Initialize data on mount
  useEffect(() => {
    initialize();
    loadConfigFromBackend().then(() => {
      const { ocr } = useLlmStore.getState();
      const dismissed = localStorage.getItem("onboarding_dismissed") === "1";
      if (!ocr.config.apiKey.trim() && !dismissed) {
        useAppStore.getState().setShowOnboarding(true);
      }
    });
  }, [initialize, loadConfigFromBackend]);

  // Global drag-drop handler
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (
          event.payload.type === "enter" ||
          event.payload.type === "over"
        ) {
          setIsDraggingFiles(true);
          return;
        }

        if (event.payload.type === "leave") {
          setIsDraggingFiles(false);
          return;
        }

        setIsDraggingFiles(false);
        const paths: string[] = (event.payload as { paths: string[] })
          .paths;
        importFiles(paths)
          .then(() => {
            triggerImportRefresh();
            navigate("/import");
          })
          .catch((err) => setError(String(err)));
      })
      .then((handler) => {
        unlisten = handler;
      })
      .catch((err) => setError(String(err)));

    return () => {
      unlisten?.();
    };
  }, [
    setIsDraggingFiles,
    setError,
    triggerImportRefresh,
    navigate,
  ]);

  // Watcher auto-import listener
  useEffect(() => {
    const unlisten = listen<WatcherImportEvent>(
      "watcher-import",
      (event) => {
        triggerImportRefresh();
        refreshInvoices();
        triggerDashboardRefresh();
      },
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [
    triggerImportRefresh,
    refreshInvoices,
    triggerDashboardRefresh,
  ]);

  // Background recognition completion listener
  useEffect(() => {
    const unlisten = listen("recognition-complete", () => {
      refreshInvoices();
      triggerImportRefresh();
      triggerDashboardRefresh();
    });
    return () => {
      unlisten.then((fn) => fn());
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
