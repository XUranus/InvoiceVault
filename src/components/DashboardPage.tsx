import React from "react";
import type { AppHealth, DashboardStats as DashboardStatsType } from "../types";
import { getDashboardStats } from "../api";
import { DashboardStats } from "./DashboardStats";

type Props = {
  health: AppHealth | null;
  error: string | null;
  refreshKey: number;
};

export function DashboardPage({ health, error, refreshKey }: Props) {
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

      {health ? (
        <div className="section" style={{ marginTop: 24 }}>
          <h3>系统信息</h3>
          <dl className="info-grid">
            <dt>数据目录</dt>
            <dd>{health.app_data_dir}</dd>
            <dt>数据库</dt>
            <dd>{health.database_path}</dd>
            <dt>迁移版本</dt>
            <dd>{health.migration_version}</dd>
          </dl>
        </div>
      ) : (
        <p className="muted">正在连接后端...</p>
      )}
    </div>
  );
}
