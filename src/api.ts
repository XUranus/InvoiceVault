import { invoke } from "@tauri-apps/api/core";
import type {
  AppHealth,
  ImportJob,
  ImportJobListResult,
  Invoice,
  InvoiceSearchParams,
  InvoiceSearchResult,
  InvoiceDetail,
  UpdateInvoiceRequest,
  UpdateInvoiceResult,
  UpdateItemsRequest,
  InvoiceItemRow,
  LlmConnectionTestResult,
  RecognizeRawFileResult,
  DedupeCheckResult,
  ExportInvoicesRequest,
  ExportResult,
  WatchDirStatus,
  AddWatchDirRequest,
  UpdateWatchDirRequest,
  DashboardStats,
  ChromaConfig,
  EmbeddingConfig,
  EmbeddingTestResult,
  SimilarResult,
  AgentSession,
  AgentMessage,
  AgentResponse,
  EventListResult,
  NotificationRow,
} from "./types";

export async function getAppHealth(): Promise<AppHealth> {
  return invoke<AppHealth>("app_health");
}

export async function importFiles(paths: string[]): Promise<ImportJob[]> {
  return invoke<ImportJob[]>("import_files", { request: { paths } });
}

export async function listImportJobs(
  page?: number,
  pageSize?: number,
): Promise<ImportJobListResult> {
  return invoke<ImportJobListResult>("list_import_jobs", { page, pageSize });
}

export async function searchInvoices(
  params: InvoiceSearchParams,
): Promise<InvoiceSearchResult> {
  return invoke<InvoiceSearchResult>("search_invoices", { params });
}

export async function getInvoiceDetail(
  invoiceId: number,
): Promise<InvoiceDetail> {
  return invoke<InvoiceDetail>("get_invoice_detail", { invoiceId });
}

export async function updateInvoice(
  request: UpdateInvoiceRequest,
): Promise<UpdateInvoiceResult> {
  return invoke<UpdateInvoiceResult>("update_invoice", { request });
}

export async function updateInvoiceItems(
  request: UpdateItemsRequest,
): Promise<InvoiceItemRow[]> {
  return invoke<InvoiceItemRow[]>("update_invoice_items", { request });
}

export async function testLlmConnection(config: {
  base_url: string;
  api_key: string;
  model: string;
  timeout_seconds: number;
}): Promise<LlmConnectionTestResult> {
  return invoke<LlmConnectionTestResult>("test_llm_connection", { config });
}

export async function recognizeRawFile(request: {
  raw_file_id: number;
  config: {
    base_url: string;
    api_key: string;
    model: string;
    timeout_seconds: number;
  };
}): Promise<RecognizeRawFileResult> {
  return invoke<RecognizeRawFileResult>("recognize_raw_file", { request });
}

export async function checkInvoiceDuplicates(
  invoiceId: number,
): Promise<DedupeCheckResult> {
  return invoke<DedupeCheckResult>("check_invoice_duplicates", { invoiceId });
}

export async function resolveDuplicate(
  dedupeId: number,
  action: string,
): Promise<void> {
  return invoke<void>("resolve_duplicate", {
    request: { dedupe_id: dedupeId, action },
  });
}

export async function exportInvoices(
  request: ExportInvoicesRequest,
): Promise<ExportResult> {
  return invoke<ExportResult>("export_invoices", { request });
}

export async function addWatchDir(
  request: AddWatchDirRequest,
): Promise<WatchDirStatus> {
  return invoke<WatchDirStatus>("add_watch_dir", { request });
}

export async function removeWatchDir(id: number): Promise<void> {
  return invoke<void>("remove_watch_dir", { id });
}

export async function listWatchDirs(): Promise<WatchDirStatus[]> {
  return invoke<WatchDirStatus[]>("list_watch_dirs");
}

export async function updateWatchDir(
  id: number,
  request: UpdateWatchDirRequest,
): Promise<WatchDirStatus> {
  return invoke<WatchDirStatus>("update_watch_dir", { id, request });
}

export async function getDashboardStats(
  dateFrom?: string,
  dateTo?: string,
): Promise<DashboardStats> {
  return invoke<DashboardStats>("get_dashboard_stats", {
    dateFrom: dateFrom ?? null,
    dateTo: dateTo ?? null,
  });
}

export async function setChromaConfig(
  config: ChromaConfig,
): Promise<void> {
  return invoke<void>("set_chroma_config", { config });
}

export async function getChromaConfig(): Promise<ChromaConfig> {
  return invoke<ChromaConfig>("get_chroma_config");
}

export async function setEmbeddingConfig(
  config: EmbeddingConfig,
): Promise<void> {
  return invoke<void>("set_embedding_config", { config });
}

export async function getEmbeddingConfig(): Promise<EmbeddingConfig> {
  return invoke<EmbeddingConfig>("get_embedding_config");
}

export async function testChromaConnection(): Promise<boolean> {
  return invoke<boolean>("test_chroma_connection");
}

export async function testEmbeddingConnection(): Promise<EmbeddingTestResult> {
  return invoke<EmbeddingTestResult>("test_embedding_connection");
}

export async function searchInvoicesSemantic(
  query: string,
  limit: number,
): Promise<SimilarResult[]> {
  return invoke<SimilarResult[]>("search_invoices_semantic", { query, limit });
}

export async function toggleWatchDir(
  id: number,
  enabled: boolean,
): Promise<WatchDirStatus> {
  return invoke<WatchDirStatus>("toggle_watch_dir", { id, enabled });
}

// Agent APIs

export async function createAgentSession(): Promise<AgentSession> {
  return invoke<AgentSession>("create_agent_session");
}

export async function listAgentSessions(): Promise<AgentSession[]> {
  return invoke<AgentSession[]>("list_agent_sessions");
}

export async function getAgentSession(
  sessionId: number,
): Promise<AgentMessage[]> {
  return invoke<AgentMessage[]>("get_agent_session", { sessionId });
}

export async function deleteAgentSession(
  sessionId: number,
): Promise<void> {
  return invoke<void>("delete_agent_session", { sessionId });
}

export async function sendAgentMessage(
  sessionId: number,
  content: string,
  config: { base_url: string; api_key: string; model: string; timeout_seconds: number },
): Promise<AgentResponse> {
  return invoke<AgentResponse>("send_agent_message", { sessionId, content, config });
}

export async function confirmAgentAction(
  sessionId: number,
  confirmed: boolean,
  extraParams: Record<string, unknown> | null,
  config: { base_url: string; api_key: string; model: string; timeout_seconds: number },
): Promise<AgentResponse> {
  return invoke<AgentResponse>("confirm_agent_action", {
    request: { session_id: sessionId, confirmed, extra_params: extraParams },
    config,
  });
}

// Event APIs

export async function listEvents(
  page?: number,
  pageSize?: number,
  eventType?: string,
): Promise<EventListResult> {
  return invoke<EventListResult>("list_events", { page, pageSize, eventType });
}

// Notification APIs

export async function listNotifications(): Promise<NotificationRow[]> {
  return invoke<NotificationRow[]>("list_notifications");
}

export async function getUnreadNotificationCount(): Promise<number> {
  return invoke<number>("get_unread_notification_count");
}

export async function markNotificationRead(id: number): Promise<void> {
  return invoke<void>("mark_notification_read", { id });
}

export async function markAllNotificationsRead(): Promise<void> {
  return invoke<void>("mark_all_notifications_read");
}

export async function dismissNotification(id: number): Promise<void> {
  return invoke<void>("dismiss_notification", { id });
}

export async function setLlmConfig(config: {
  base_url: string;
  api_key: string;
  model: string;
}): Promise<void> {
  return invoke<void>("set_llm_config", { config });
}
