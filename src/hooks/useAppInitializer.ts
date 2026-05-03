import { useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import type { WatcherImportEvent } from "../types";
import {
  importFiles,
  getRecognitionQueueStatus,
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
  const incrementInvoiceBadgeCount = useAppStore(
    (s) => s.incrementInvoiceBadgeCount,
  );
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
    loadConfigFromBackend();
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
        const count =
          event.payload.imported_count ??
          event.payload.jobs?.length ??
          0;
        incrementInvoiceBadgeCount(count);
      },
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [
    triggerImportRefresh,
    refreshInvoices,
    triggerDashboardRefresh,
    incrementInvoiceBadgeCount,
  ]);

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
}
