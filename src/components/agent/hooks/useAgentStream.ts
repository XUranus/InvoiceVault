import { useAgentStore } from "./useAgentStore";

export function useAgentStream() {
  const streamState = useAgentStore((s) => s.streamState);

  return {
    streamState,
    isActive: streamState !== null && streamState.phase !== "done" && streamState.phase !== "error",
  };
}
