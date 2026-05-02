import React from "react";
import type { DashboardStats as DashboardStatsType } from "../types";
import { getDashboardStats } from "../api";
import { DashboardStats } from "./DashboardStats";

type Props = {
  error: string | null;
  refreshKey: number;
};

export function DashboardPage({ error, refreshKey }: Props) {
  const [stats, setStats] = React.useState<DashboardStatsType | null>(null);
  const [statsError, setStatsError] = React.useState<string | null>(null);

  React.useEffect(() => {
    getDashboardStats()
      .then(setStats)
      .catch((err) => setStatsError(String(err)));
  }, [refreshKey]);

  return (
    <div className="page">
      <h2 className="page-title">仪表盘</h2>

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
