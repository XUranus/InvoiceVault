import React from "react";
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
} from "../api";
import { importStatusMeta, toneClass } from "../status";
import { DashboardStats } from "./DashboardStats";
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
    return (value / 10000).toFixed(1) + " 万元";
  }
  return value.toFixed(2) + " 元";
}

function formatJobTime(value: string): string {
  const numericValue = Number(value);
  if (Number.isFinite(numericValue) && numericValue > 1000000000000) {
    return new Date(numericValue).toLocaleString();
  }
  return value;
}

function jobTitle(job: ImportJob): string {
  return job.original_name || job.current_name || job.source_path;
}

function queueTotal(queueStatus: RecognitionQueueStatus | null): number {
  return queueStatus ? queueStatus.pending + queueStatus.running : 0;
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

export function DashboardPage() {
  const error = useAppStore((s) => s.error);
  const refreshKey = useRefreshStore((s) => s.dashboardKey);
  const navigateToInvoice = useNavigateToInvoice();
  const [stats, setStats] = React.useState<DashboardStatsType | null>(null);
  const [statsError, setStatsError] = React.useState<string | null>(null);
  const [queueStatus, setQueueStatus] = React.useState<RecognitionQueueStatus | null>(null);
  const [recentJobs, setRecentJobs] = React.useState<ImportJob[]>([]);
  const [operationsError, setOperationsError] = React.useState<string | null>(null);
  const [dateRange, setDateRange] = React.useState<DateRange>("all");
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
    Promise.all([getRecognitionQueueStatus(), listImportJobs(1, 5)])
      .then(([queue, jobs]) => {
        setQueueStatus(queue);
        setRecentJobs(jobs.jobs.slice(0, 5));
      })
      .catch((err) => {
        setQueueStatus(null);
        setRecentJobs([]);
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

  const failedRecentJobs = recentJobs.filter((job) => job.status === "failed").length;

  const handleOpenImportedJob = async (job: ImportJob) => {
    if (!canOpenImportedJob(job)) return;
    if (job.invoice_id) {
      navigateToInvoice(job.invoice_id);
      return;
    }
    if (!job.raw_file_id) return;
    try {
      const invoiceId = await getInvoiceIdByRawFile(job.raw_file_id);
      if (invoiceId) {
        navigateToInvoice(invoiceId);
      } else {
        setOperationsError("该导入任务还没有生成发票详情。");
      }
    } catch (err) {
      setOperationsError(String(err));
    }
  };

  return (
    <div className="page">
      <div className="page-header-row">
        <h2 className="page-title">仪表盘</h2>
        <div className="date-range-bar">
          {DATE_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              className={`btn-sm ${dateRange === opt.value ? "btn-primary" : "btn-outline"}`}
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
            <input
              type="date"
              value={customFrom}
              onChange={(e) => setCustomFrom(e.target.value)}
            />
          </label>
          <label className="form-field">
            <span>到</span>
            <input
              type="date"
              value={customTo}
              onChange={(e) => setCustomTo(e.target.value)}
            />
          </label>
        </div>
      )}

      {error ? <div className="alert alert-error">{error}</div> : null}
      {statsError ? <div className="alert alert-error">{statsError}</div> : null}
      {operationsError ? <div className="alert alert-error">{operationsError}</div> : null}

      {stats ? (
        <>
          <div className="dashboard-work-grid">
            <div className="dashboard-work-card dashboard-work-card-primary">
              <span className="dashboard-work-label">待确认</span>
              <span className="dashboard-work-value">{stats.pending_count}</span>
              <span className="dashboard-work-meta">需要人工复核的发票</span>
            </div>
            <div className="dashboard-work-card">
              <span className="dashboard-work-label">识别队列</span>
              <span className="dashboard-work-value">{queueTotal(queueStatus)}</span>
              <span className="dashboard-work-meta">
                处理中 {queueStatus?.running ?? "--"} / 等待 {queueStatus?.pending ?? "--"}
              </span>
            </div>
            <div className="dashboard-work-card">
              <span className="dashboard-work-label">最近导入失败</span>
              <span className="dashboard-work-value">{failedRecentJobs}</span>
              <span className="dashboard-work-meta">最近 {recentJobs.length} 次导入</span>
            </div>
            <div className="dashboard-work-card">
              <span className="dashboard-work-label">本月新增</span>
              <span className="dashboard-work-value">{stats.this_month_count}</span>
              <span className="dashboard-work-meta">
                {formatAmount(stats.this_month_amount)}
              </span>
            </div>
            <div className="dashboard-work-card">
              <span className="dashboard-work-label">LLM 用量</span>
              <span className="dashboard-work-value">
                {usage ? usage.total_tokens.toLocaleString() : "--"}
              </span>
              <span className="dashboard-work-meta">
                调用 {usage?.total_calls ?? 0} 次 | 预估{" "}
                {usage && priceConfig ? formatCost(estimateCost(usage, priceConfig)) : "--"}
              </span>
            </div>
          </div>

          <div className="dashboard-main-grid">
            <section className="dashboard-panel">
              <div className="dashboard-panel-header">
                <div>
                  <h3>业务概览</h3>
                  <p>当前筛选范围内的入库和金额汇总</p>
                </div>
              </div>
              <div className="dashboard-summary-grid">
                <div>
                  <span className="dashboard-summary-value">{stats.total_invoices}</span>
                  <span className="dashboard-summary-label">已入库发票</span>
                </div>
                <div>
                  <span className="dashboard-summary-value">
                    {formatAmount(stats.total_amount)}
                  </span>
                  <span className="dashboard-summary-label">金额合计</span>
                </div>
                <div>
                  <span className="dashboard-summary-value">{stats.duplicate_count}</span>
                  <span className="dashboard-summary-label">重复风险</span>
                </div>
                <div>
                  <span className="dashboard-summary-value">
                    {(stats.average_confidence * 100).toFixed(0)}%
                  </span>
                  <span className="dashboard-summary-label">平均置信度</span>
                </div>
              </div>
            </section>

            <section className="dashboard-panel">
              <div className="dashboard-panel-header">
                <div>
                  <h3>最近导入</h3>
                </div>
              </div>
              {recentJobs.length > 0 ? (
                <div className="dashboard-job-table-wrap">
                  <table className="dashboard-job-table">
                    <tbody>
                      {recentJobs.map((job) => {
                        const meta = importStatusMeta(job.status);
                        const canOpen = canOpenImportedJob(job);
                        return (
                          <tr
                            key={job.id}
                            className={canOpen ? "dashboard-job-row-clickable" : ""}
                            onClick={() => handleOpenImportedJob(job)}
                            title={canOpen ? "打开发票详情" : undefined}
                          >
                            <td>
                              <span className="dashboard-job-title" title={jobTitle(job)}>
                                {jobTitle(job)}
                              </span>
                              {job.error_message ? (
                                <span className="dashboard-job-meta" title={job.error_message}>
                                  {job.error_message}
                                </span>
                              ) : null}
                            </td>
                            <td className="dashboard-job-time">{formatJobTime(job.created_at)}</td>
                            <td>
                              <span className={`mini-tag ${toneClass(meta.tone)}`}>{meta.label}</span>
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              ) : (
                <p className="dashboard-empty">导入发票后这里会显示最近任务。</p>
              )}
            </section>
          </div>

          <DashboardStats stats={stats} />
        </>
      ) : (
        <div className="stat-cards">
          <div className="stat-card">
            <span className="stat-value">--</span>
            <span className="stat-label">已入库发票</span>
          </div>
          <div className="stat-card">
            <span className="stat-value">--</span>
            <span className="stat-label">金额合计</span>
          </div>
          <div className="stat-card">
            <span className="stat-value">--</span>
            <span className="stat-label">本月新增</span>
          </div>
          <div className="stat-card">
            <span className="stat-value">--</span>
            <span className="stat-label">待确认</span>
          </div>
        </div>
      )}
    </div>
  );
}

export default DashboardPage;
