import React from "react";
import type { DashboardStats as DashboardStatsType } from "../types";
import { getDashboardStats } from "../api";
import { DashboardStats } from "./DashboardStats";

type DateRange = "all" | "this_month" | "last_month" | "last_3m" | "custom";

type Props = {
  error: string | null;
  refreshKey: number;
};

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

export function DashboardPage({ error, refreshKey }: Props) {
  const [stats, setStats] = React.useState<DashboardStatsType | null>(null);
  const [statsError, setStatsError] = React.useState<string | null>(null);
  const [dateRange, setDateRange] = React.useState<DateRange>("all");
  const [customFrom, setCustomFrom] = React.useState("");
  const [customTo, setCustomTo] = React.useState("");

  React.useEffect(() => {
    const params = dateRangeToParams(dateRange, customFrom, customTo);
    getDashboardStats(params.from, params.to)
      .then(setStats)
      .catch((err) => setStatsError(String(err)));
  }, [refreshKey, dateRange, customFrom, customTo]);

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

      {stats ? (
        <DashboardStats stats={stats} />
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
