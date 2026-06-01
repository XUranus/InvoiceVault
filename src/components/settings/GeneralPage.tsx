import React from "react";
import type { BadgeConfig } from "../../types";
import {
  getBadgeConfig,
  setBadgeConfig,
  regenerateAllDuplicates,
} from "../../api";
import { useAppStore } from "../../stores/appStore";
import { useRefreshStore } from "../../stores/refreshStore";
import { Sun, Moon, Trash2 } from "lucide-react";

export function GeneralPage() {
  const theme = useAppStore((s) => s.theme);
  const toggleTheme = useAppStore((s) => s.toggleTheme);

  const refreshInvoices = useAppStore((s) => s.refreshInvoices);
  const triggerInvoicesRefresh = useRefreshStore((s) => s.triggerInvoicesRefresh);

  const [regenerating, setRegenerating] = React.useState(false);
  const [regenResult, setRegenResult] = React.useState<string | null>(null);

  // --- Badge Config ---
  const [badgeConfig, setBadgeConfigState] = React.useState<BadgeConfig>({ groups: [] });
  const [badgeOptionDrafts, setBadgeOptionDrafts] = React.useState<Record<number, string>>({});
  const [savingBadgeConfig, setSavingBadgeConfig] = React.useState(false);
  const [badgeConfigMessage, setBadgeConfigMessage] = React.useState<string | null>(null);

  const handleRegenerate = async () => {
    if (regenerating) return;
    setRegenerating(true);
    setRegenResult(null);
    try {
      const count = await regenerateAllDuplicates();
      setRegenResult(`完成：已重新检测 ${count} 张发票的重复状态`);
      refreshInvoices();
      triggerInvoicesRefresh();
    } catch (err) {
      setRegenResult(`失败：${String(err)}`);
    } finally {
      setRegenerating(false);
    }
  };

  const addBadgeGroup = () => {
    setBadgeConfigState((prev) => ({
      groups: [...prev.groups, { name: "", options: [] }],
    }));
  };

  const removeBadgeGroup = (index: number) => {
    setBadgeConfigState((prev) => ({
      groups: prev.groups.filter((_, i) => i !== index),
    }));
  };

  const updateBadgeGroupName = (index: number, name: string) => {
    setBadgeConfigState((prev) => ({
      groups: prev.groups.map((g, i) => (i === index ? { ...g, name } : g)),
    }));
  };

  const addBadgeOption = (groupIndex: number) => {
    const draft = (badgeOptionDrafts[groupIndex] ?? "").trim();
    if (!draft) return;
    setBadgeConfigState((prev) => ({
      groups: prev.groups.map((g, i) =>
        i === groupIndex ? { ...g, options: [...g.options, draft] } : g,
      ),
    }));
    setBadgeOptionDrafts((prev) => ({ ...prev, [groupIndex]: "" }));
  };

  const removeBadgeOption = (groupIndex: number, optionIndex: number) => {
    setBadgeConfigState((prev) => ({
      groups: prev.groups.map((g, i) =>
        i === groupIndex
          ? { ...g, options: g.options.filter((_, oi) => oi !== optionIndex) }
          : g,
      ),
    }));
  };

  const handleBadgeOptionKeyDown = (e: React.KeyboardEvent, groupIndex: number) => {
    if (e.key === "Enter") {
      e.preventDefault();
      addBadgeOption(groupIndex);
    }
  };

  const handleSaveBadgeConfig = async () => {
    setSavingBadgeConfig(true);
    setBadgeConfigMessage(null);
    try {
      await setBadgeConfig(badgeConfig);
      setBadgeConfigMessage("已保存");
    } catch (err) {
      setBadgeConfigMessage(`保存失败: ${String(err)}`);
    } finally {
      setSavingBadgeConfig(false);
    }
  };

  React.useEffect(() => {
    let cancelled = false;
    Promise.all([
      getBadgeConfig().catch(() => null),
    ]).then(([badge]) => {
      if (cancelled) return;
      if (badge) setBadgeConfigState(badge);
    });
    return () => { cancelled = true; };
  }, []);

  return (
    <>
      {/* Appearance */}
      <div className="section">
        <h3>外观</h3>
        <p className="section-desc">
          当前: {theme === "dark" ? "暗色主题" : "亮色主题"}
        </p>
        <button className="btn-primary" onClick={toggleTheme}>
          {theme === "dark" ? <><Sun size={16} /> 切换到亮色主题</> : <><Moon size={16} /> 切换到暗色主题</>}
        </button>
      </div>

      {/* Duplicate Detection */}
      <div className="section">
        <h3>重复检测</h3>
        <p className="section-desc">
          清除所有已有的重复检测结果，根据当前阈值重新匹配所有发票并生成告警。
        </p>
        <button
          className="btn-primary"
          onClick={handleRegenerate}
          disabled={regenerating}
        >
          {regenerating ? "检测中…" : "重新检测重复"}
        </button>
        {regenResult && (
          <p className="section-desc" style={{ marginTop: 8 }}>{regenResult}</p>
        )}
      </div>

      {/* Badge Config */}
      <div className="section badge-config-section">
        <div className="section-header">
          <h3>自定义 Badge</h3>
          <button
            className="btn-small"
            type="button"
            onClick={addBadgeGroup}
          >
            添加分组
          </button>
        </div>
        <p className="section-desc">
          配置后可在发票详情页为单张发票选择标签。每个分组一张发票只能选择一个值。
        </p>

        <div className="badge-config-list">
          {badgeConfig.groups.map((group, groupIndex) => (
            <div className="badge-config-card" key={groupIndex}>
              <div className="badge-config-card-header">
                <label className="form-field">
                  <span>分组名称</span>
                  <input
                    value={group.name}
                    onChange={(e) => updateBadgeGroupName(groupIndex, e.target.value)}
                    placeholder="例如：电商"
                  />
                </label>
                <button
                  className="btn-icon-danger badge-group-remove"
                  type="button"
                  onClick={() => removeBadgeGroup(groupIndex)}
                  aria-label={`删除分组 ${group.name || groupIndex + 1}`}
                  title="删除分组"
                >
                  <Trash2 size={16} />
                </button>
              </div>
              <div className="badge-option-editor">
                <div className="badge-option-input-row">
                  <input
                    className="badge-option-input"
                    value={badgeOptionDrafts[groupIndex] ?? ""}
                    onChange={(e) => setBadgeOptionDrafts((prev) => ({ ...prev, [groupIndex]: e.target.value }))}
                    onKeyDown={(e) => handleBadgeOptionKeyDown(e, groupIndex)}
                    placeholder="输入 Badge 名称，按 Enter 添加"
                  />
                </div>
                <div className="badge-chip-list">
                  {group.options.map((option, optionIndex) => {
                    const label = option.trim();
                    if (!label) return null;
                    return (
                      <span className="badge-chip" key={`${label}-${optionIndex}`}>
                        <span className="badge-chip-label">{label}</span>
                        <button
                          className="badge-chip-remove"
                          type="button"
                          aria-label={`删除 ${label}`}
                          title="删除"
                          onClick={() => removeBadgeOption(groupIndex, optionIndex)}
                        >
                          ×
                        </button>
                      </span>
                    );
                  })}
                  {group.options.every((option) => !option.trim()) ? (
                    <span className="muted badge-chip-empty">暂无 Badge</span>
                  ) : null}
                </div>
              </div>
            </div>
          ))}
          {badgeConfig.groups.length === 0 ? (
            <p className="muted">暂未配置 Badge 分组。</p>
          ) : null}
        </div>

        <div style={{ display: "flex", alignItems: "center", gap: 12, marginTop: 12 }}>
          <button
            className="btn-primary"
            type="button"
            onClick={handleSaveBadgeConfig}
            disabled={savingBadgeConfig}
          >
            {savingBadgeConfig ? "保存中..." : "保存 Badge 配置"}
          </button>
          {badgeConfigMessage ? (
            <span className="badge-config-message">{badgeConfigMessage}</span>
          ) : null}
        </div>
      </div>
    </>
  );
}

export default GeneralPage;
