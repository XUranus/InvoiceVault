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
  InvoiceBadgeSelection,
  BadgeConfig,
  LlmConnectionTestResult,
  RecognizeRawFileResult,
  DedupeCheckResult,
  ResolveDuplicateResult,
  ExportInvoicesRequest,
  ExportResult,
  WatchDirStatus,
  AddWatchDirRequest,
  UpdateWatchDirRequest,
  EmailSource,
  AddEmailSourceRequest,
  UpdateEmailSourceRequest,
  EmailTestResult,
  EmailSyncResult,
  DashboardStats,
  ChromaConfig,
  LocalEmbeddingStatus,
  EmbeddingTestResult,
  SimilarResult,
  AgentSession,
  AgentAttachment,
  AgentArtifact,
  AgentTask,
  AgentMessage,
  AgentResponse,
  EventListResult,
  RecognitionQueueStatus,
  ExportLogsResult,
  CleanupStorageResult,
  ExternalDependencyStatus,
  LlmUsageStats,
  PriceConfig,
  DiagnosticConfig,
  DiagnosticResult,
  MergeInvoicesResult,
  PdfReportResult,
  PdfReportRequest,
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

export async function markInvoiceViewed(invoiceId: number): Promise<boolean> {
  return invoke<boolean>("mark_invoice_viewed", { invoiceId });
}

export async function countUnviewedInvoices(): Promise<number> {
  return invoke<number>("count_unviewed_invoices");
}

export async function openInvoiceRawFileInBrowser(invoiceId: number): Promise<void> {
  return invoke<void>("open_invoice_raw_file_in_browser", { invoiceId });
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

export async function getBadgeConfig(): Promise<BadgeConfig> {
  return invoke<BadgeConfig>("get_badge_config");
}

export async function setBadgeConfig(config: BadgeConfig): Promise<void> {
  return invoke<void>("set_badge_config", { config });
}

export async function getTheme(): Promise<string> {
  return invoke<string>("get_theme");
}

export async function setTheme(theme: string): Promise<void> {
  return invoke<void>("set_theme", { theme });
}

export async function setInvoiceBadge(
  invoiceId: number,
  groupName: string,
  value: string | null,
): Promise<InvoiceBadgeSelection[]> {
  return invoke<InvoiceBadgeSelection[]>("set_invoice_badge", {
    invoiceId,
    groupName,
    value,
  });
}

export async function batchUpdateInvoices(request: {
  ids: number[];
  status?: string | null;
  category?: string | null;
}): Promise<Invoice[]> {
  return invoke<Invoice[]>("batch_update_invoices", { request });
}

export async function batchDeleteInvoices(ids: number[]): Promise<number> {
  return invoke<number>("batch_delete_invoices", { ids });
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
): Promise<ResolveDuplicateResult> {
  return invoke<ResolveDuplicateResult>("resolve_duplicate", {
    request: { dedupe_id: dedupeId, action },
  });
}

export async function regenerateAllDuplicates(): Promise<number> {
  return invoke<number>("regenerate_all_duplicates");
}

export async function exportInvoices(
  request: ExportInvoicesRequest,
): Promise<ExportResult> {
  return invoke<ExportResult>("export_invoices", { request });
}

export async function mergeInvoices(
  targetInvoiceId: number,
  sourceInvoiceIds: number[],
): Promise<MergeInvoicesResult> {
  return invoke<MergeInvoicesResult>("merge_invoices", {
    target_invoice_id: targetInvoiceId,
    source_invoice_ids: sourceInvoiceIds,
  });
}

export async function exportPdfReport(
  request: PdfReportRequest,
): Promise<PdfReportResult> {
  return invoke<PdfReportResult>("export_pdf_report", { request });
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

export async function setEmbeddingEnabled(
  enabled: boolean,
): Promise<void> {
  return invoke<void>("set_embedding_enabled", { enabled });
}

export async function getEmbeddingStatus(): Promise<LocalEmbeddingStatus> {
  return invoke<LocalEmbeddingStatus>("get_embedding_status");
}

export async function downloadEmbeddingModel(): Promise<LocalEmbeddingStatus> {
  return invoke<LocalEmbeddingStatus>("download_embedding_model");
}

export async function testChromaConnection(): Promise<boolean> {
  return invoke<boolean>("test_chroma_connection");
}

export async function testEmbeddingConnection(): Promise<EmbeddingTestResult> {
  return invoke<EmbeddingTestResult>("test_embedding_connection");
}

export type RegenerateEmbeddingsResult = {
  total_invoices: number;
  success_count: number;
  failure_count: number;
};

export async function regenerateAllEmbeddings(): Promise<RegenerateEmbeddingsResult> {
  return invoke<RegenerateEmbeddingsResult>("regenerate_all_embeddings");
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

// Email Source APIs

export async function addEmailSource(
  request: AddEmailSourceRequest,
): Promise<EmailSource> {
  return invoke<EmailSource>("add_email_source", { request });
}

export async function updateEmailSource(
  id: number,
  request: UpdateEmailSourceRequest,
): Promise<EmailSource> {
  return invoke<EmailSource>("update_email_source", { id, request });
}

export async function removeEmailSource(id: number): Promise<void> {
  return invoke<void>("remove_email_source", { id });
}

export async function listEmailSources(): Promise<EmailSource[]> {
  return invoke<EmailSource[]>("list_email_sources");
}

export async function toggleEmailSource(
  id: number,
  enabled: boolean,
): Promise<EmailSource> {
  return invoke<EmailSource>("toggle_email_source", { id, enabled });
}

export async function syncEmailSource(id: number): Promise<EmailSyncResult> {
  return invoke<EmailSyncResult>("sync_email_source", { id });
}

export async function syncAllEmailSources(): Promise<EmailSyncResult[]> {
  return invoke<EmailSyncResult[]>("sync_all_email_sources");
}

export async function testEmailConnection(config: {
  protocol: string;
  host: string;
  port: number;
  username: string;
  password: string;
  authMethod: string;
  useSsl: boolean;
  folder: string;
}): Promise<EmailTestResult> {
  return invoke<EmailTestResult>("test_email_connection", config);
}

export async function analyzeEmailError(
  errorMessage: string,
): Promise<string | null> {
  return invoke<string | null>("analyze_email_error", { errorMessage });
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
  attachmentIds: number[] = [],
): Promise<AgentResponse> {
  return invoke<AgentResponse>("send_agent_message", {
    sessionId,
    content,
    config,
    attachmentIds,
  });
}

export async function sendAgentMessageStream(
  streamId: string,
  sessionId: number,
  content: string,
  config: { base_url: string; api_key: string; model: string; timeout_seconds: number },
  attachmentIds: number[] = [],
): Promise<AgentResponse> {
  return invoke<AgentResponse>("send_agent_message_stream", {
    streamId,
    sessionId,
    content,
    config,
    attachmentIds,
  });
}

export async function attachAgentFile(
  sessionId: number,
  path: string,
): Promise<AgentAttachment> {
  return invoke<AgentAttachment>("attach_agent_file", { sessionId, path });
}

export async function listAgentAttachments(
  sessionId: number,
): Promise<AgentAttachment[]> {
  return invoke<AgentAttachment[]>("list_agent_attachments", { sessionId });
}

export async function listAgentTasks(
  sessionId: number,
): Promise<AgentTask[]> {
  return invoke<AgentTask[]>("list_agent_tasks", { sessionId });
}

export async function listAgentArtifacts(
  sessionId: number,
): Promise<AgentArtifact[]> {
  return invoke<AgentArtifact[]>("list_agent_artifacts", { sessionId });
}

export async function openAgentArtifactFile(
  sessionId: number,
  artifactId: number,
): Promise<void> {
  return invoke<void>("open_agent_artifact_file", { sessionId, artifactId });
}

export async function openAgentArtifactFolder(
  sessionId: number,
  artifactId: number,
): Promise<void> {
  return invoke<void>("open_agent_artifact_folder", { sessionId, artifactId });
}

export async function deleteAgentArtifact(
  sessionId: number,
  artifactId: number,
): Promise<void> {
  return invoke<void>("delete_agent_artifact", { sessionId, artifactId });
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

export async function confirmAgentActionStream(
  streamId: string,
  sessionId: number,
  confirmed: boolean,
  extraParams: Record<string, unknown> | null,
  config: { base_url: string; api_key: string; model: string; timeout_seconds: number },
): Promise<AgentResponse> {
  return invoke<AgentResponse>("confirm_agent_action_stream", {
    streamId,
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

// Event read/unread APIs

export async function getUnreadEventCount(): Promise<number> {
  return invoke<number>("get_unread_event_count");
}

export async function getUnreadFailedImportEventCount(): Promise<number> {
  return invoke<number>("get_unread_failed_import_event_count");
}

export async function markEventRead(id: number): Promise<void> {
  return invoke<void>("mark_event_read", { id });
}

export async function markAllEventsRead(): Promise<void> {
  return invoke<void>("mark_all_events_read");
}

export async function setLlmConfig(config: {
  base_url: string;
  api_key: string;
  model: string;
  scnet_ocr_api_key?: string;
}): Promise<void> {
  return invoke<void>("set_llm_config", { config });
}

export type LlmConfigResponse = {
  base_url: string;
  api_key: string;
  model: string;
  timeout_seconds?: number;
  scnet_ocr_api_key?: string;
} | null;

export async function getLlmConfig(): Promise<LlmConfigResponse> {
  return invoke<LlmConfigResponse>("get_llm_config");
}


export async function setLlmAuditEnabled(enabled: boolean): Promise<void> {
  return invoke<void>("set_llm_audit_enabled", { enabled });
}

export async function getLlmAuditEnabled(): Promise<boolean> {
  return invoke<boolean>("get_llm_audit_enabled");
}

export async function getRecognitionQueueStatus(): Promise<RecognitionQueueStatus> {
  return invoke<RecognitionQueueStatus>("get_recognition_queue_status");
}

export async function rawFileHasInvoices(rawFileId: number): Promise<boolean> {
  return invoke<boolean>("raw_file_has_invoices", { rawFileId });
}

export async function getInvoiceIdByRawFile(rawFileId: number): Promise<number | null> {
  return invoke<number | null>("get_invoice_id_by_raw_file", { rawFileId });
}

export async function deleteAllEvents(): Promise<number> {
  return invoke<number>("delete_all_events");
}

export async function deleteImportJob(jobId: number): Promise<void> {
  return invoke<void>("delete_import_job", { jobId });
}

export async function exportLogs(outputPath: string): Promise<ExportLogsResult> {
  return invoke<ExportLogsResult>("export_logs", { outputPath });
}

export async function cleanupStorage(): Promise<CleanupStorageResult> {
  return invoke<CleanupStorageResult>("cleanup_storage");
}

export async function exportBackup(outputPath: string): Promise<ExportLogsResult> {
  return invoke<ExportLogsResult>("export_backup", { outputPath });
}

export async function checkExternalDependencies(): Promise<ExternalDependencyStatus[]> {
  return invoke<ExternalDependencyStatus[]>("check_external_dependencies");
}

export async function getLlmUsage(
  dateFrom?: string,
  dateTo?: string,
): Promise<LlmUsageStats> {
  return invoke<LlmUsageStats>("get_llm_usage", { dateFrom, dateTo });
}

export async function getPriceConfig(): Promise<PriceConfig> {
  return invoke<PriceConfig>("get_price_config");
}

export async function setPriceConfig(config: PriceConfig): Promise<void> {
  return invoke<void>("set_price_config", { config });
}

export async function getDiagnosticConfig(): Promise<DiagnosticConfig> {
  return invoke<DiagnosticConfig>("get_diagnostic_config");
}

export async function setDiagnosticConfig(
  config: DiagnosticConfig,
): Promise<void> {
  return invoke<void>("set_diagnostic_config", { config });
}

export async function runLlmDiagnostic(): Promise<DiagnosticResult> {
  return invoke<DiagnosticResult>("run_llm_diagnostic");
}
