import React from "react";
import { useNavigate } from "react-router-dom";
import type {
  DashboardStats as DashboardStatsType,
  ImportJob,
  RecognitionQueueStatus,
  LlmUsageStats,
  PriceConfig,
} from "../types";
import {
  getDashboardStats,
  getInvoiceIdByRawFile,
  getRecognitionQueueStatus,
  listImportJobs,
  getLlmUsage,
  getPriceConfig,
  getUnreadFailedImportEventCount,
} from "../api";
import { importStatusMeta, toneClass } from "../status";
import { DashboardCharts } from "./DashboardStats";
import { useAppStore } from "../stores/appStore";
import { useRefreshStore } from "../stores/refreshStore";
import { useNavigateToInvoice } from "../hooks/useNavigateToInvoice";

type DateRange = "all" | "this_month" | "last_month" | "last_3m" | "custom";

function dateRangeToParams(
  range: DateRange,
  customFrom: string,
  customTo: string,
): { from?: string; to?: string } {
  const now = new Date();
  const fmt = (d: Date) => d.toISOString().slice(0, 10);
  switch (range) {
    case "this_month":
      return { from: fmt(new Date(now.getFullYear(), now.getMonth(), 1)) };
    case "last_month":
      return {
        from: fmt(new Date(now.getFullYear(), now.getMonth() - 1, 1)),
        to: fmt(new Date(now.getFullYear(), now.getMonth(), 0)),
      };
    case "last_3m":
      return { from: fmt(new Date(now.getFullYear(), now.getMonth() - 2, 1)) };
    case "custom":
      return { from: customFrom || undefined, to: customTo || undefined };
    default:
      return {};
  }
}

const DATE_OPTIONS: { value: DateRange; label: string }[] = [
  { value: "all", label: "全部" },
  { value: "this_month", label: "本月" },
  { value: "last_month", label: "上月" },
  { value: "last_3m", label: "近三月" },
  { value: "custom", label: "自定义" },
];

function formatAmount(value: number): string {
  if (value >= 10000) {
    return (value / 10000).toFixed(1) + " 万";
  }
  return "¥" + value.toFixed(2);
}

function formatJobTime(value: string): string {
  const date = new Date(value);
  if (!Number.isNaN(date.getTime())) {
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMin = Math.floor(diffMs / 60000);
    if (diffMin < 1) return "刚刚";
    if (diffMin < 60) return `${diffMin} 分钟前`;
    const diffHr = Math.floor(diffMin / 60);
    if (diffHr < 24) return `${diffHr} 小时前`;
    return date.toLocaleDateString();
  }
  return value;
}

function jobTitle(job: ImportJob): string {
  return job.original_name || job.current_name || job.source_path;
}

function canOpenImportedJob(job: ImportJob): boolean {
  return ["imported", "completed", "recognized"].includes(job.status) && (
    Boolean(job.invoice_id) || Boolean(job.raw_file_id)
  );
}

function estimateCost(usage: LlmUsageStats, price: PriceConfig): number {
  const llmInputCost = (usage.total_prompt_tokens / 1000) * price.llm_input_price_per_1k;
  const llmOutputCost = (usage.total_completion_tokens / 1000) * price.llm_output_price_per_1k;
  return llmInputCost + llmOutputCost;
}

function formatCost(cost: number): string {
  if (cost < 0.01) return "< ¥0.01";
  return `¥${cost.toFixed(2)}`;
}

function rangeLabel(range: DateRange): string {
  switch (range) {
    case "this_month": return "本月";
    case "last_month": return "上月";
    case "last_3m": return "近三月";
    case "custom": return "所选时段";
    default: return "全部";
  }
}

function duplicatePairs(count: number): number {
  return Math.ceil(count / 2);
}

function buildPageSummary(stats: DashboardStatsType, range: DateRange): string {
  const label = rangeLabel(range);
  const parts: string[] = [];
  parts.push(`${label}已处理 ${stats.total_invoices} 张发票`);
  parts.push(`累计 ${formatAmount(stats.total_amount)}`);
  if (stats.average_confidence > 0) {
    parts.push(`平均置信度 ${(stats.average_confidence * 100).toFixed(0)}%`);
  }
  if (stats.duplicate_count > 0) {
    parts.push(`${duplicatePairs(stats.duplicate_count)} 对重复风险`);
  }
  return parts.join("，");
}

function buildInsight(stats: DashboardStatsType): string | null {
  const issues: string[] = [];
  if (stats.duplicate_count > 0) issues.push(`${duplicatePairs(stats.duplicate_count)} 对疑似重复`);
  if (stats.pending_count > 0) issues.push(`${stats.pending_count} 张待复核`);
  if (issues.length > 0) {
    return `检测到 ${issues.join("、")}，建议优先核查`;
  }
  if (stats.total_invoices > 0) {
    return "识别质量稳定，数据状态良好";
  }
  return null;
}

export function DashboardPage() {
  const error = useAppStore((s) => s.error);
  const refreshKey = useRefreshStore((s) => s.dashboardKey);
  const navigate = useNavigate();
  const navigateToInvoice = useNavigateToInvoice();
  const [stats, setStats] = React.useState<DashboardStatsType | null>(null);
  const [statsError, setStatsError] = React.useState<string | null>(null);
  const [queueStatus, setQueueStatus] = React.useState<RecognitionQueueStatus | null>(null);
  const [recentJobs, setRecentJobs] = React.useState<ImportJob[]>([]);
  const [unreadFailedCount, setUnreadFailedCount] = React.useState(0);
  const [operationsError, setOperationsError] = React.useState<string | null>(null);
  const [dateRange, setDateRange] = React.useState<DateRange>("this_month");
  const [customFrom, setCustomFrom] = React.useState("");
  const [customTo, setCustomTo] = React.useState("");
  const [usage, setUsage] = React.useState<LlmUsageStats | null>(null);
  const [priceConfig, setPriceConfig] = React.useState<PriceConfig | null>(null);

  React.useEffect(() => {
    const params = dateRangeToParams(dateRange, customFrom, customTo);
    setStatsError(null);
    getDashboardStats(params.from, params.to)
      .then(setStats)
      .catch((err) => {
        setStats(null);
        setStatsError(String(err));
      });
  }, [refreshKey, dateRange, customFrom, customTo]);

  React.useEffect(() => {
    setOperationsError(null);
    Promise.all([getRecognitionQueueStatus(), listImportJobs(1, 5), getUnreadFailedImportEventCount()])
      .then(([queue, jobs, failed]) => {
        setQueueStatus(queue);
        setRecentJobs(jobs.jobs.slice(0, 5));
        setUnreadFailedCount(failed);
      })
      .catch((err) => {
        setQueueStatus(null);
        setRecentJobs([]);
        setUnreadFailedCount(0);
        setOperationsError(String(err));
      });
  }, [refreshKey]);

  React.useEffect(() => {
    Promise.all([getLlmUsage(), getPriceConfig()])
      .then(([u, p]) => {
        setUsage(u);
        setPriceConfig(p);
      })
      .catch(() => {});
  }, [refreshKey]);

  const queueTotal = queueStatus ? queueStatus.pending + queueStatus.running : 0;
  const hasIssues = stats && (stats.pending_count > 0 || unreadFailedCount > 0 || stats.duplicate_count > 0);

  const handleOpenImportedJob = async (job: ImportJob) => {
    if (!canOpenImportedJob(job)) return;
    if (job.invoice_id) {
      navigateToInvoice(job.invoice_id, "/dashboard");
      return;
    }
    if (!job.raw_file_id) return;
    try {
      const invoiceId = await getInvoiceIdByRawFile(job.raw_file_id);
      if (invoiceId) {
        navigateToInvoice(invoiceId, "/dashboard");
      } else {
        setOperationsError("该导入任务还没有生成发票详情。");
      }
    } catch (err) {
      setOperationsError(String(err));
    }
  };

  return (
    <div className="page dashboard-page">
      {/* Header */}
      <div className="dashboard-header">
        <div className="dashboard-header-top">
          <h2 className="page-title">仪表盘</h2>
          <div className="date-range-bar">
            {DATE_OPTIONS.map((opt) => (
              <button
                key={opt.value}
                className={`date-range-btn ${dateRange === opt.value ? "is-active" : ""}`}
                onClick={() => setDateRange(opt.value)}
              >
                {opt.label}
              </button>
            ))}
          </div>
        </div>

        {dateRange === "custom" && (
          <div className="date-range-custom">
            <label className="form-field">
              <span>从</span>
              <input type="date" value={customFrom} onChange={(e) => setCustomFrom(e.target.value)} />
            </label>
            <label className="form-field">
              <span>到</span>
              <input type="date" value={customTo} onChange={(e) => setCustomTo(e.target.value)} />
            </label>
          </div>
        )}

        {error ? <div className="alert alert-error">{error}</div> : null}
        {statsError ? <div className="alert alert-error">{statsError}</div> : null}
        {operationsError ? <div className="alert alert-error">{operationsError}</div> : null}
      </div>

      {stats ? (
        <>
          {/* Page-level summary */}
          <p className="dashboard-page-summary">{buildPageSummary(stats, dateRange)}</p>

          {/* Row 1: Hero + System status */}
          <div className="dashboard-row-hero">
            {/* Hero summary card */}
            <section className="dashboard-hero-card">
              <div className="dashboard-hero-head">
                <div>
                  <div className="dashboard-hero-eyebrow">{rangeLabel(dateRange)}</div>
                  <h3 className="dashboard-hero-title">业务概览</h3>
                </div>
              </div>
              <p className="dashboard-hero-summary">
                已处理 <strong>{stats.total_invoices}</strong> 张发票，
                累计 <strong>{formatAmount(stats.total_amount)}</strong>
                {stats.duplicate_count > 0
                  ? <>，含 <strong className="text-warn">{duplicatePairs(stats.duplicate_count)}</strong> 对重复风险</>
                  : null}
              </p>
              <div className="dashboard-hero-metrics">
                <div className="dashboard-hero-metric">
                  <span className="dashboard-hero-value">{stats.this_month_count || stats.total_invoices}</span>
                  <span className="dashboard-hero-label">发票数</span>
                </div>
                <div className="dashboard-hero-metric">
                  <span className="dashboard-hero-value">{formatAmount(stats.this_month_amount || stats.total_amount)}</span>
                  <span className="dashboard-hero-label">金额</span>
                </div>
                <div className="dashboard-hero-metric">
                  <span className="dashboard-hero-value">{(stats.average_confidence * 100).toFixed(0)}%</span>
                  <span className="dashboard-hero-label">置信度</span>
                </div>
                <div className="dashboard-hero-metric">
                  <span className={`dashboard-hero-value ${stats.duplicate_count > 0 ? "text-warn" : ""}`}>{duplicatePairs(stats.duplicate_count)}</span>
                  <span className="dashboard-hero-label">重复对数</span>
                </div>
              </div>
              {buildInsight(stats) ? (
                <p className="dashboard-hero-insight">{buildInsight(stats)}</p>
              ) : null}
            </section>

            {/* System status card */}
            <section className="dashboard-status-card">
              <div className="dashboard-status-head">
                <span className="dashboard-status-eyebrow">系统状态</span>
              </div>
              <div className="dashboard-status-rows">
                <div className="dashboard-status-row">
                  <span className={`dashboard-status-dot ${stats.pending_count > 0 ? "dot-warning" : "dot-idle"}`} />
                  <span className="dashboard-status-label">待确认</span>
                  <span className={`dashboard-status-value ${stats.pending_count === 0 ? "is-idle" : ""}`}>
                    {stats.pending_count > 0 ? stats.pending_count : "无"}
                  </span>
                </div>
                <div className="dashboard-status-row">
                  <span className={`dashboard-status-dot ${unreadFailedCount > 0 ? "dot-danger" : "dot-idle"}`} />
                  <span className="dashboard-status-label">导入失败</span>
                  <span className={`dashboard-status-value ${unreadFailedCount === 0 ? "is-idle" : ""}`}>
                    {unreadFailedCount > 0 ? unreadFailedCount : "无"}
                  </span>
                </div>
                <div className="dashboard-status-row">
                  <span className={`dashboard-status-dot ${queueTotal > 0 ? "dot-primary" : "dot-idle"}`} />
                  <span className="dashboard-status-label">识别队列</span>
                  <span className={`dashboard-status-value ${queueTotal === 0 ? "is-idle" : ""}`}>
                    {queueTotal > 0 ? queueTotal : "空闲"}
                  </span>
                </div>
              </div>
              {queueStatus ? (
                <div className="dashboard-status-meta">
                  处理中 {queueStatus.running} · 等待 {queueStatus.pending}
                </div>
              ) : null}
            </section>
          </div>

          {/* Row 2: Action items */}
          {hasIssues ? (
            <section className="dashboard-action-panel">
              <div className="dashboard-action-header">
                <h3 className="dashboard-section-title">待处理事项</h3>
                <span className="dashboard-action-count">
                  {stats.pending_count + unreadFailedCount + duplicatePairs(stats.duplicate_count)} 项
                </span>
              </div>
              <div className="dashboard-action-rows">
                {stats.pending_count > 0 && (
                  <div className="dashboard-action-row">
                    <span className="dashboard-action-indicator indicator-warning" />
                    <div className="dashboard-action-body">
                      <span className="dashboard-action-label">{stats.pending_count} 张发票待复核</span>
                      <span className="dashboard-action-hint">识别完成但尚未人工确认</span>
                    </div>
                    <button className="dashboard-action-btn" onClick={() => navigateToInvoice(0)}>开始复核</button>
                  </div>
                )}
                {unreadFailedCount > 0 && (
                  <div className="dashboard-action-row">
                    <span className="dashboard-action-indicator indicator-danger" />
                    <div className="dashboard-action-body">
                      <span className="dashboard-action-label">{unreadFailedCount} 个导入任务失败</span>
                      <span className="dashboard-action-hint">点击查看详情</span>
                    </div>
                    <button className="dashboard-action-btn" onClick={() => navigate("/events")}>查看详情</button>
                  </div>
                )}
                {stats.duplicate_count > 0 && (
                  <div className="dashboard-action-row">
                    <span className="dashboard-action-indicator indicator-warning" />
                    <div className="dashboard-action-body">
                      <span className="dashboard-action-label">{duplicatePairs(stats.duplicate_count)} 对发票疑似重复</span>
                      <span className="dashboard-action-hint">需要确认是否保留或合并</span>
                    </div>
                    <button className="dashboard-action-btn" onClick={() => navigateToInvoice(0)}>核查重复发票</button>
                  </div>
                )}
              </div>
            </section>
          ) : null}

          {/* Row 3: Trend chart full-width */}
          <DashboardCharts stats={stats} />

          {/* Row 4: Recent imports + Breakdown */}
          <div className="dashboard-row-bottom">
            {/* Recent imports */}
            <section className="dashboard-panel">
              <h3 className="dashboard-section-title">最近导入</h3>
              {recentJobs.length > 0 ? (
                <div className="dashboard-job-list">
                  {recentJobs.map((job) => {
                    const meta = importStatusMeta(job.status);
                    const canOpen = canOpenImportedJob(job);
                    return (
                      <div
                        key={job.id}
                        className={`dashboard-job-row ${canOpen ? "is-clickable" : ""}`}
                        onClick={() => handleOpenImportedJob(job)}
                        title={canOpen ? "打开发票详情" : undefined}
                      >
                        <span className="dashboard-job-name" title={jobTitle(job)}>
                          {jobTitle(job)}
                        </span>
                        <span className="dashboard-job-time">{formatJobTime(job.created_at)}</span>
                        <span className={`mini-tag ${toneClass(meta.tone)}`}>{meta.label}</span>
                      </div>
                    );
                  })}
                </div>
              ) : (
                <p className="dashboard-empty">导入发票后这里会显示最近任务。</p>
              )}
            </section>

            {/* Type distribution - horizontal bars */}
            {stats.by_type.length > 0 ? (
              <section className="dashboard-panel">
                <h3 className="dashboard-section-title">类型分布</h3>
                <div className="dashboard-type-bars">
                  {stats.by_type.slice(0, 6).map((item, i) => {
                    const total = stats.by_type.reduce((s, b) => s + b.count, 0);
                    const pct = total > 0 ? (item.count / total) * 100 : 0;
                    return (
                      <div className="dashboard-type-row" key={item.label}>
                        <div className="dashboard-type-row-header">
                          <span className="dashboard-type-label">{item.label}</span>
                          <span className="dashboard-type-count">{item.count} 张 · {pct.toFixed(0)}%</span>
                        </div>
                        <div className="dashboard-type-track">
                          <div className="dashboard-type-fill" style={{ width: `${pct}%`, background: `var(--chart-series-${(i % 6) + 1})` }} />
                        </div>
                      </div>
                    );
                  })}
                </div>
              </section>
            ) : null}

            {/* Top sellers - compact list */}
            {stats.top_sellers.length > 0 ? (
              <section className="dashboard-panel">
                <h3 className="dashboard-section-title">Top 供应商</h3>
                <div className="dashboard-seller-list">
                  {stats.top_sellers.slice(0, 5).map((s, i) => (
                    <div className="dashboard-seller-row" key={s.seller_name}>
                      <span className="dashboard-seller-rank">{i + 1}</span>
                      <span className="dashboard-seller-name">{s.seller_name}</span>
                      <span className="dashboard-seller-count">{s.count} 张</span>
                      <span className="dashboard-seller-amount">{formatAmount(s.amount)}</span>
                    </div>
                  ))}
                </div>
              </section>
            ) : null}
          </div>

          {/* LLM usage footer */}
          {usage ? (
            <div className="dashboard-footer-bar">
              <span className="dashboard-footer-text">
                LLM · {usage.total_calls} 次调用 · {usage.total_tokens.toLocaleString()} tokens
                {priceConfig ? ` · 预估 ${formatCost(estimateCost(usage, priceConfig))}` : ""}
              </span>
            </div>
          ) : null}
        </>
      ) : (
        <div className="dashboard-skeleton">
          <div className="dashboard-skeleton-hero" />
          <div className="dashboard-skeleton-row" />
        </div>
      )}
    </div>
  );
}

export default DashboardPage;
