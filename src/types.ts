export type AppHealth = {
  app_data_dir: string;
  database_path: string;
  migration_version: number;
};

export type ImportJob = {
  id: number;
  raw_file_id: number | null;
  source_path: string;
  original_name: string | null;
  current_name: string | null;
  status: string;
  sha256: string | null;
  storage_path: string | null;
  mime_type: string | null;
  error_message: string | null;
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
  source_page_range: string | null;
  confidence: number | null;
  status: string;
  duplicate_status: string;
  created_at: string;
  updated_at: string;
  items: InvoiceItemRow[];
  raw_file_name: string | null;
  raw_file_mime: string | null;
  thumbnail_path: string | null;
  extraction_model: string | null;
  extraction_provider: string | null;
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

export type WatchDirConfig = {
  id: number;
  path: string;
  extensions: string;
  recursive: boolean;
  enabled: boolean;
  stable_wait_ms: number;
  archive_after_import: boolean;
  archive_path: string | null;
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
};

export type UpdateWatchDirRequest = {
  path?: string;
  extensions?: string;
  recursive?: boolean;
  stable_wait_ms?: number;
  archive_after_import?: boolean;
  archive_path?: string | null;
};

export type WatcherImportEvent = {
  watch_dir_id: number;
  watch_dir_path: string;
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

export type EmbeddingConfig = {
  base_url: string;
  api_key: string;
  model: string;
  enabled: boolean;
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

export type AgentMessage = {
  id: number;
  session_id: number;
  role: string;
  content: string;
  tool_call_json: string | null;
  created_at: string;
};

export type PendingConfirmation = {
  tool_name: string;
  arguments: Record<string, unknown>;
  message: string;
};

export type AgentResponse = {
  messages: AgentMessage[];
  pending_confirmation: PendingConfirmation | null;
};

// Event types

export type EventRow = {
  id: number;
  event_type: string;
  title: string;
  description: string;
  status: string;
  reference_type: string | null;
  reference_id: number | null;
  metadata_json: string | null;
  created_at: string;
};

export type EventListResult = {
  events: EventRow[];
  total_count: number;
  page: number;
  page_size: number;
  total_pages: number;
};

// Notification types

export type NotificationRow = {
  id: number;
  level: "info" | "warning" | "error";
  title: string;
  message: string;
  is_read: boolean;
  reference_type: string | null;
  reference_id: number | null;
  created_at: string;
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
