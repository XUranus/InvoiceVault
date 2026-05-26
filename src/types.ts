export type AppHealth = {
  app_data_dir: string;
  database_path: string;
  migration_version: number;
};

export type ImportJob = {
  id: number;
  raw_file_id: number | null;
  invoice_id: number | null;
  source_path: string;
  original_name: string | null;
  current_name: string | null;
  status: string;
  sha256: string | null;
  storage_path: string | null;
  mime_type: string | null;
  error_message: string | null;
  source_type: string;
  created_at: string;
  updated_at: string;
};

export type ImportJobListResult = {
  jobs: ImportJob[];
  total_count: number;
  page: number;
  page_size: number;
  total_pages: number;
};

export type LlmConnectionTestResult = {
  model: string;
  duration_ms: number;
  response_preview: string;
};

export type Invoice = {
  id: number;
  raw_file_id: number;
  raw_file_mime: string | null;
  invoice_type: string | null;
  invoice_code: string | null;
  invoice_number: string | null;
  issue_date: string | null;
  seller_name: string | null;
  buyer_name: string | null;
  currency: string;
  total_amount: string | null;
  category: string | null;
  source_page_range: string | null;
  confidence: number | null;
  status: string;
  duplicate_status: string;
  created_at: string;
  updated_at: string;
  viewed_at: string | null;
  item_names: string | null;
  badges: InvoiceBadgeSelection[];
};

export type RecognizeRawFileResult = {
  invoices: Invoice[];
  model: string;
  duration_ms: number;
  response_preview: string;
  page_count: number;
  thumbnail_paths: string[];
};

export type InvoiceSearchParams = {
  query?: string;
  invoice_type?: string;
  seller_name?: string;
  buyer_name?: string;
  invoice_number?: string;
  date_from?: string;
  date_to?: string;
  amount_min?: string;
  amount_max?: string;
  category?: string;
  tag?: string;
  status?: string;
  duplicate_status?: string;
  sort_by?: string;
  sort_order?: string;
  page?: number;
  page_size?: number;
};

export type InvoiceSearchResult = {
  invoices: Invoice[];
  total_count: number;
  page: number;
  page_size: number;
  total_pages: number;
};

export type TagOption = {
  label: string;
  count: number;
};

export type InvoiceItemRow = {
  id: number;
  name: string;
  specification: string | null;
  unit: string | null;
  quantity: string | null;
  unit_price: string | null;
  amount: string | null;
  tax_rate: string | null;
  tax_amount: string | null;
};

export type InvoiceBadgeSelection = {
  group_name: string;
  value: string;
};

export type BadgeGroupConfig = {
  name: string;
  options: string[];
};

export type BadgeConfig = {
  groups: BadgeGroupConfig[];
};

export type InvoiceDetail = {
  id: number;
  raw_file_id: number;
  invoice_type: string | null;
  invoice_code: string | null;
  invoice_number: string | null;
  issue_date: string | null;
  seller_name: string | null;
  seller_tax_id: string | null;
  buyer_name: string | null;
  buyer_tax_id: string | null;
  currency: string;
  amount_without_tax: string | null;
  tax_amount: string | null;
  total_amount: string | null;
  category: string | null;
  remarks: string | null;
  extra_fields: string | null;
  source_page_range: string | null;
  confidence: number | null;
  status: string;
  duplicate_status: string;
  created_at: string;
  updated_at: string;
  viewed_at: string | null;
  items: InvoiceItemRow[];
  raw_file_name: string | null;
  raw_file_mime: string | null;
  raw_file_path: string | null;
  thumbnail_path: string | null;
  extraction_model: string | null;
  extraction_provider: string | null;
  badges: InvoiceBadgeSelection[];
  source_type: string | null;
};

export type FieldError = {
  field: string;
  message: string;
};

export type UpdateInvoiceRequest = {
  id: number;
  invoice_type?: string | null;
  invoice_code?: string | null;
  invoice_number?: string | null;
  issue_date?: string | null;
  seller_name?: string | null;
  seller_tax_id?: string | null;
  buyer_name?: string | null;
  buyer_tax_id?: string | null;
  currency?: string | null;
  amount_without_tax?: string | null;
  tax_amount?: string | null;
  total_amount?: string | null;
  category?: string | null;
  remarks?: string | null;
  confidence?: number | null;
  status?: string | null;
  extra_fields?: Record<string, unknown> | null;
};

export type UpdateInvoiceResult = {
  invoice: Invoice;
  errors: FieldError[];
};

export type InvoiceItemChange = {
  action: "add" | "update" | "delete";
  id?: number;
  name: string;
  specification?: string | null;
  unit?: string | null;
  quantity?: string | null;
  unit_price?: string | null;
  amount?: string | null;
  tax_rate?: string | null;
  tax_amount?: string | null;
};

export type UpdateItemsRequest = {
  invoice_id: number;
  items: InvoiceItemChange[];
};

export type DedupeCandidate = {
  id: number;
  candidate_invoice_id: number;
  seller_name: string | null;
  invoice_number: string | null;
  issue_date: string | null;
  total_amount: string | null;
  score: number;
  reason: string;
  status: string;
};

export type DedupeCheckResult = {
  invoice_id: number;
  candidates: DedupeCandidate[];
  has_exact_duplicate: boolean;
};

export type ResolveDuplicateResult = {
  action: string;
  deleted_invoice_id: number | null;
};

export type ExportInvoicesRequest = {
  format: string;
  output_path: string;
  invoice_ids?: number[];
  columns?: string[];
  date_from?: string;
  date_to?: string;
};

export type ExportResult = {
  file_path: string;
  row_count: number;
  format: string;
  byte_size: number;
  columns: string[];
};

export type MergeInvoicesResult = {
  merged_invoice: Invoice;
  merged_from_ids: number[];
  total_items_merged: number;
};

export type PdfReportResult = {
  file_path: string;
  invoice_count: number;
  byte_size: number;
};

export type PdfReportRequest = {
  output_path: string;
  invoice_ids?: number[];
  date_from?: string;
  date_to?: string;
};

export type WatchDirConfig = {
  id: number;
  path: string;
  extensions: string;
  recursive: boolean;
  enabled: boolean;
  stable_wait_ms: number;
  archive_after_import: boolean;
  archive_path: string | null;
  name_keywords: string;
  max_file_age_days: number;
  created_at: string;
  updated_at: string;
};

export type WatchDirStatus = WatchDirConfig & {
  running: boolean;
  error: string | null;
};

export type AddWatchDirRequest = {
  path: string;
  extensions?: string;
  recursive?: boolean;
  stable_wait_ms?: number;
  name_keywords?: string;
  max_file_age_days?: number;
};

export type UpdateWatchDirRequest = {
  path?: string;
  extensions?: string;
  recursive?: boolean;
  stable_wait_ms?: number;
  archive_after_import?: boolean;
  archive_path?: string | null;
  name_keywords?: string;
  max_file_age_days?: number;
};

export type WatcherImportEvent = {
  watch_dir_id: number;
  watch_dir_path: string;
  imported_count: number;
  jobs: ImportJob[];
};

export type EmailSource = {
  id: number;
  name: string;
  protocol: string;
  imap_host: string;
  imap_port: number;
  username: string;
  password: string;
  auth_method: string;
  use_ssl: boolean;
  folder: string;
  name_keywords: string;
  max_email_age_days: number;
  enabled: boolean;
  last_uid: number;
  poll_interval_seconds: number;
  processed_uidls: string;
  last_sync_at: string | null;
  status: string;
  error_message: string | null;
  created_at: string;
  updated_at: string;
};

export type AddEmailSourceRequest = {
  name?: string;
  protocol?: string;
  imap_host: string;
  imap_port?: number;
  username: string;
  password: string;
  auth_method?: string;
  use_ssl?: boolean;
  folder?: string;
  name_keywords?: string;
  max_email_age_days?: number;
  poll_interval_seconds?: number;
};

export type UpdateEmailSourceRequest = {
  name?: string;
  protocol?: string;
  imap_host?: string;
  imap_port?: number;
  username?: string;
  password?: string;
  auth_method?: string;
  use_ssl?: boolean;
  folder?: string;
  name_keywords?: string;
  max_email_age_days?: number;
  poll_interval_seconds?: number;
  enabled?: boolean;
};

export type EmailTestResult = {
  success: boolean;
  message: string;
  folder_count: number | null;
};

export type EmailSyncResult = {
  source_id: number;
  fetched_count: number;
  imported_count: number;
  jobs: ImportJob[];
};

export type MonthlyTrendPoint = {
  month: string;
  count: number;
  amount: number;
};

export type BreakdownItem = {
  label: string;
  count: number;
  amount: number;
};

export type TopSellerItem = {
  seller_name: string;
  count: number;
  amount: number;
};

export type DashboardStats = {
  total_invoices: number;
  total_amount: number;
  currency: string;
  average_confidence: number;
  this_month_count: number;
  this_month_amount: number;
  pending_count: number;
  duplicate_count: number;
  monthly_trend: MonthlyTrendPoint[];
  by_type: BreakdownItem[];
  by_status: BreakdownItem[];
  top_sellers: TopSellerItem[];
};

export type ChromaConfig = {
  enabled: boolean;
};

export type LocalEmbeddingStatus = {
  enabled: boolean;
  model_present: boolean;
  model_loaded: boolean;
  model_dir: string | null;
  dimensions: number | null;
};

export type EmbeddingTestResult = {
  model: string;
  dimensions: number;
  duration_ms: number;
};

export type SimilarResult = {
  invoice_id: number;
  similarity: number;
  metadata: Record<string, string>;
};

// Agent types

export type AgentSession = {
  id: number;
  title: string;
  created_at: string;
  updated_at: string;
};

export type AgentAttachment = {
  id: number;
  session_id: number;
  message_id: number | null;
  original_name: string;
  mime_type: string | null;
  byte_size: number;
  storage_path: string;
  created_at: string;
};

export type AgentTask = {
  id: number;
  session_id: number;
  tool_name: string;
  status: string;
  input_json: string | null;
  result_json: string | null;
  error_message: string | null;
  created_at: string;
  updated_at: string;
  completed_at: string | null;
};

export type AgentArtifact = {
  id: number;
  session_id: number;
  task_id: number | null;
  artifact_type: string;
  title: string;
  file_path: string | null;
  mime_type: string | null;
  byte_size: number | null;
  metadata_json: string | null;
  created_at: string;
};

export type AgentMessage = {
  id: number;
  session_id: number;
  role: string;
  content: string;
  tool_call_json: string | null;
  tool_call_id: string | null;
  created_at: string;
  attachments: AgentAttachment[];
};

export type ConfirmOption = {
  label: string;
  value: string;
  style?: "primary" | "secondary" | "danger";
};

export type PendingConfirmation = {
  tool_name: string;
  arguments: Record<string, unknown>;
  message: string;
  options?: ConfirmOption[];
};

export type AgentResponse = {
  messages: AgentMessage[];
  pending_confirmation: PendingConfirmation | null;
};

export type AgentStreamEvent =
  | {
      stream_id: string;
      session_id: number;
      type: "started";
    }
  | {
      stream_id: string;
      session_id: number;
      type: "assistant_delta";
      delta: string;
    }
  | {
      stream_id: string;
      session_id: number;
      type: "tool_call";
      tool_name: string;
    }
  | {
      stream_id: string;
      session_id: number;
      type: "tool_result";
      tool_name: string;
    }
  | {
      stream_id: string;
      session_id: number;
      type: "pending_confirmation";
      pending_confirmation: PendingConfirmation;
    }
  | {
      stream_id: string;
      session_id: number;
      type: "finished";
    }
  | {
      stream_id: string;
      session_id: number;
      type: "error";
      message: string;
    };

// Event types

export type EventRow = {
  id: number;
  event_type: string;
  title: string;
  description: string;
  status: string;
  is_read: boolean;
  reference_type: string | null;
  reference_id: number | null;
  metadata_json: string | null;
  created_at: string;
};

export type EventListResult = {
  events: EventRow[];
  total_count: number;
  unread_count: number;
  page: number;
  page_size: number;
  total_pages: number;
};

export type RecognitionQueueStatus = {
  pending: number;
  running: number;
  max_concurrent: number;
};

export type ExportLogsResult = {
  file_path: string;
  byte_size: number;
};

export type CleanupStorageResult = {
  files_removed: number;
  db_records_removed: number;
  bytes_freed: number;
};

export type ExternalDependencyStatus = {
  name: string;
  command: string;
  available: boolean;
  version: string | null;
  error: string | null;
};

export type LlmUsageStats = {
  total_calls: number;
  llm_calls: number;
  embedding_calls: number;
  total_prompt_tokens: number;
  total_completion_tokens: number;
  total_tokens: number;
  this_month_calls: number;
  this_month_tokens: number;
};

export type PriceConfig = {
  llm_input_price_per_1k: number;
  llm_output_price_per_1k: number;
  embedding_input_price_per_1k: number;
  embedding_output_price_per_1k: number;
};

export type GroundTruth = {
  invoice_type: string | null;
  invoice_code: string | null;
  invoice_number: string | null;
  issue_date: string | null;
  seller_name: string | null;
  buyer_name: string | null;
  total_amount: number | null;
  amount_without_tax: number | null;
  tax_amount: number | null;
  items_count: number | null;
};

export type DiagnosticConfig = {
  test_image_path: string;
  ground_truth: GroundTruth;
  enabled: boolean;
};

export type DiagnosticStep = {
  name: string;
  passed: boolean;
  duration_ms: number;
  message: string;
  details: string | null;
};

export type DiagnosticResult = {
  steps: DiagnosticStep[];
  score: number | null;
  all_passed: boolean;
};
