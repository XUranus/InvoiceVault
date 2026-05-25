import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import type { AgentStreamEvent } from "../../../types";
import { useAgentStore } from "./useAgentStore";

export function useAgentStream() {
  const handleStreamEvent = useAgentStore((s) => s.handleStreamEvent);
  const activeSessionId = useAgentStore((s) => s.activeSessionId);
  const streamState = useAgentStore((s) => s.streamState);

  const sessionIdRef = useRef(activeSessionId);
  const streamStateRef = useRef(streamState);

  useEffect(() => {
    sessionIdRef.current = activeSessionId;
  }, [activeSessionId]);

  useEffect(() => {
    streamStateRef.current = streamState;
  }, [streamState]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    listen<AgentStreamEvent>("agent://stream", (event) => {
      const payload = event.payload;
      // Only handle events for the active session
      if (payload.session_id !== sessionIdRef.current) return;
      handleStreamEvent(payload);
    })
      .then((cleanup) => {
        unlisten = cleanup;
      })
      .catch((err) => {
        console.error("Failed to set up agent stream listener:", err);
      });

    return () => {
      unlisten?.();
    };
  }, [handleStreamEvent]);

  return {
    streamState,
    isActive: streamState !== null && streamState.phase !== "done" && streamState.phase !== "error",
  };
}
