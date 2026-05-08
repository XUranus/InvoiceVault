import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from "recharts";
import type { DashboardStats as DashboardStatsType } from "../types";

const chartGrid = "var(--color-chart-grid)";
const chartAxis = "var(--color-chart-axis)";
const chartText = "var(--color-chart-tooltip-text)";
const chartPrimary = "var(--chart-series-1)";
const chartSecondary = "var(--chart-series-2)";
const chartTooltip = {
  background: "var(--color-chart-tooltip-bg)",
  border: "1px solid var(--color-chart-tooltip-border)",
  borderRadius: 8,
  color: chartText,
  fontSize: 12,
};

type Props = {
  stats: DashboardStatsType;
};

export function DashboardCharts({ stats }: Props) {
  if (stats.total_invoices === 0) return null;
  if (stats.monthly_trend.length === 0) return null;

  const singlePoint = stats.monthly_trend.length === 1;

  if (singlePoint) {
    const point = stats.monthly_trend[0];
    return (
      <section className="dashboard-panel dashboard-trend-panel">
        <div className="dashboard-trend-header">
          <h3 className="dashboard-section-title">月度趋势</h3>
        </div>
        <div className="dashboard-trend-single">
          <div className="dashboard-trend-single-metric">
            <span className="dashboard-trend-single-value">{point.count}</span>
            <span className="dashboard-trend-single-label">{point.month} 发票数</span>
          </div>
          <div className="dashboard-trend-single-metric">
            <span className="dashboard-trend-single-value">
              {point.amount >= 10000
                ? (point.amount / 10000).toFixed(1) + " 万"
                : "¥" + point.amount.toFixed(2)}
            </span>
            <span className="dashboard-trend-single-label">{point.month} 金额</span>
          </div>
        </div>
        <p className="dashboard-trend-insight">数据不足两个月，趋势图将在更多数据积累后展示。</p>
      </section>
    );
  }

  return (
    <section className="dashboard-panel dashboard-trend-panel">
      <div className="dashboard-trend-header">
        <h3 className="dashboard-section-title">月度趋势</h3>
        <div className="dashboard-trend-legend">
          <span className="dashboard-trend-legend-item">
            <span className="dashboard-trend-legend-line" style={{ background: chartPrimary }} />
            发票数
          </span>
          <span className="dashboard-trend-legend-item">
            <span className="dashboard-trend-legend-line" style={{ background: chartSecondary }} />
            金额
          </span>
        </div>
      </div>
      <ResponsiveContainer width="100%" height={200}>
        <LineChart data={stats.monthly_trend} margin={{ top: 4, right: 8, bottom: 0, left: -12 }}>
          <CartesianGrid strokeDasharray="3 3" stroke={chartGrid} vertical={false} />
          <XAxis dataKey="month" stroke={chartAxis} fontSize={11} tickLine={false} axisLine={false} />
          <YAxis yAxisId="left" stroke={chartPrimary} fontSize={11} width={36} tickLine={false} axisLine={false} />
          <YAxis yAxisId="right" orientation="right" stroke={chartSecondary} fontSize={11} width={36} tickLine={false} axisLine={false} />
          <Tooltip
            contentStyle={chartTooltip}
            labelStyle={{ color: chartText, fontWeight: 600 }}
            itemStyle={{ color: chartText }}
          />
          <Line
            yAxisId="left"
            type="monotone"
            dataKey="count"
            name="发票数"
            stroke={chartPrimary}
            strokeWidth={2}
            dot={false}
            activeDot={{ r: 4, fill: chartPrimary, stroke: "var(--color-surface-elevated)", strokeWidth: 2 }}
          />
          <Line
            yAxisId="right"
            type="monotone"
            dataKey="amount"
            name="金额"
            stroke={chartSecondary}
            strokeWidth={2}
            dot={false}
            activeDot={{ r: 4, fill: chartSecondary, stroke: "var(--color-surface-elevated)", strokeWidth: 2 }}
          />
        </LineChart>
      </ResponsiveContainer>
    </section>
  );
}
