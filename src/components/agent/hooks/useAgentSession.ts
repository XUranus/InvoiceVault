import { useEffect } from "react";
import { useAgentStore } from "./useAgentStore";

export function useAgentSession() {
  const store = useAgentStore();

  useEffect(() => {
    store.loadSessions();
  }, []);

  const createNewSession = async () => {
    return await store.createSession();
  };

  const deleteSession = async (id: number) => {
    await store.deleteSession(id);
  };

  const switchSession = (id: number | null) => {
    store.setActiveSession(id);
  };

  return {
    sessions: store.sessions,
    activeSessionId: store.activeSessionId,
    loading: store.loading,
    createNewSession,
    deleteSession,
    switchSession,
  };
}
