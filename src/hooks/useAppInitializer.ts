import { useCallback, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import {
  importFiles,
  pollDroppedFiles,
  getRecognitionQueueStatus,
  syncAllEmailSources,
  listEmailSources,
  frontendHeartbeat,
} from "../api";
import { useAppStore } from "../stores/appStore";
import { useLlmStore } from "../stores/llmStore";
import { useRefreshStore } from "../stores/refreshStore";

// Mutex guard: prevents duplicate import when multiple drop events fire
// for a single physical file drop (DOM + native handler overlap).
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
      console.log("[drag-drop] handleDroppedFiles called, paths:", paths, "inProgress:", dropImportInProgress);
      if (paths.length === 0) {
        console.warn("[drag-drop] dropped with empty paths, ignoring");
        return;
      }
      if (dropImportInProgress) {
        console.warn("[drag-drop] import already in progress, skipping duplicate");
        return;
      }
      dropImportInProgress = true;
      setIsDraggingFiles(false);
      setDragImportPaths(paths);
      navigate("/import");
      console.log("[drag-drop] invoking importFiles IPC with paths:", paths);
      importFiles(paths)
        .then((jobs) => {
          console.log("[drag-drop] importFiles succeeded, jobs:", jobs);
          triggerImportRefresh();
        })
        .catch((err) => {
          console.error("[drag-drop] importFiles failed:", err);
          setError(String(err));
        })
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

  // Poll backend for native drag-drop file paths (bypasses broken Tauri event system)
  useEffect(() => {
    let stopped = false;
    const poll = async () => {
      if (stopped || dropImportInProgress) return;
      try {
        const paths = await pollDroppedFiles();
        if (paths.length > 0) {
          console.log("[drag-drop] polled dropped files:", paths);
          handleDroppedFiles(paths);
        }
      } catch { /* ignore poll errors */ }
    };
    const interval = setInterval(poll, 500);
    return () => { stopped = true; clearInterval(interval); };
  }, [handleDroppedFiles]);

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
