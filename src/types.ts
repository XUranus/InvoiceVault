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
};

export type ExportResult = {
  file_path: string;
  row_count: number;
  format: string;
  byte_size: number;
};
