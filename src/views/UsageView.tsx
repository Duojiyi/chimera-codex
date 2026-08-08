import { useCallback, useEffect, useRef, useState } from "react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import {
  CircleAlert,
  DatabaseBackup,
  RefreshCw,
  RotateCcw,
  ShieldCheck,
} from "lucide-react";
import { toast } from "sonner";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { usageApi } from "@/lib/api/usage";
import type {
  CodexUsageRebuildResult,
  DailyStats,
  ModelStats,
  UsageSummary,
} from "@/types/usage";

const runningInTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** How many models the 模型词元分布 panel shows. */
export const USAGE_TOP_MODEL_COUNT = 3;

/**
 * Rank models by the quantity the panel actually renders — tokens.
 *
 * The backend sorts by `total_cost DESC` (correct for the cost-columned
 * ModelStatsTable, so it is not changed there). Taking the first N of that
 * order here ranked by *cost* while the panel is labelled 模型词元分布 and
 * prints 词元 counts, so a cheap-but-chatty model outranked an expensive-but-
 * quiet one on screen. Worse, a model with no pricing entry has cost 0 and can
 * never enter the top N no matter how many tokens it burns — which makes the
 * panel look like a hardcoded model list.
 *
 * Sorted copy, not in place: the caller's array is React state.
 */
export function topModelsByTokens(
  models: ModelStats[],
  count = USAGE_TOP_MODEL_COUNT,
): ModelStats[] {
  return [...models]
    .sort((a, b) => b.totalTokens - a.totalTokens)
    .slice(0, count);
}

export function UsageView() {
  const [summary, setSummary] = useState<UsageSummary | null>(null);
  const [trends, setTrends] = useState<DailyStats[]>([]);
  const [models, setModels] = useState<ModelStats[]>([]);
  const [range, setRange] = useState<"today" | "7d" | "30d">("30d");
  const [error, setError] = useState("");
  const [syncing, setSyncing] = useState(runningInTauri);
  const [rebuilding, setRebuilding] = useState(false);
  const [rebuildDialogOpen, setRebuildDialogOpen] = useState(false);
  const [rebuildResult, setRebuildResult] =
    useState<CodexUsageRebuildResult | null>(null);
  const [rebuildError, setRebuildError] = useState("");
  const [rangeLoading, setRangeLoading] = useState(false);
  const [syncNote, setSyncNote] = useState(
    runningInTauri
      ? "正在读取 Codex 本机会话记录"
      : "浏览器预览不会读取本机会话数据",
  );
  const initialUsageLoadStarted = useRef(false);
  const usageRequestId = useRef(0);

  const loadUsage = useCallback(
    async (selectedRange: "today" | "7d" | "30d", syncSessions: boolean) => {
      const requestId = ++usageRequestId.current;
      if (!runningInTauri) {
        setSummary(null);
        setTrends([]);
        setModels([]);
        setSyncing(false);
        setRangeLoading(false);
        setSyncNote("浏览器预览不会读取本机会话数据");
        return;
      }
      if (syncSessions) setSyncing(true);
      else setRangeLoading(true);
      setError("");
      if (syncSessions) {
        try {
          const result = await usageApi.syncCodexSessionUsage();
          setSyncNote(
            result.errors.length
              ? `已读取 ${result.filesScanned} 个文件，${result.errors.length} 项未能导入`
              : `已同步 ${result.filesScanned} 个本机会话文件`,
          );
        } catch (reason) {
          setSyncNote("本机会话同步失败，正在显示已有记录");
          setError(String(reason));
        }
      }
      const now = new Date();
      const end = Math.floor(now.getTime() / 1000);
      const days = selectedRange === "7d" ? 7 : 30;
      const start =
        selectedRange === "today"
          ? Math.floor(
              new Date(
                now.getFullYear(),
                now.getMonth(),
                now.getDate(),
              ).getTime() / 1000,
            )
          : end - days * 24 * 60 * 60;
      try {
        const [nextSummary, nextTrends, nextModels] = await Promise.all([
          usageApi.getUsageSummary(start, end, "codex"),
          usageApi.getUsageTrends(start, end, "codex"),
          usageApi.getModelStats(start, end, "codex"),
        ]);
        if (requestId !== usageRequestId.current) return;
        setSummary(nextSummary);
        setTrends(nextTrends);
        // Keep every model: the top-N slice happens at render time, and the
        // percentage denominator needs the full set to be a real share.
        setModels(nextModels);
      } catch (reason) {
        if (requestId !== usageRequestId.current) return;
        setError(String(reason));
      } finally {
        if (requestId === usageRequestId.current) {
          setSyncing(false);
          setRangeLoading(false);
        }
      }
    },
    [],
  );

  const rebuildUsage = useCallback(async () => {
    if (!runningInTauri || rebuilding) return;

    setRebuildDialogOpen(false);
    setRebuilding(true);
    setRebuildError("");
    setRebuildResult(null);
    try {
      const result = await usageApi.rebuildCodexUsage();
      setRebuildResult(result);
      setSyncNote("Codex 用量已从本机会话重新构建");
      toast.success("Codex 用量重建完成", {
        description: `扫描 ${result.filesScanned} 个文件，导入 ${result.imported} 条记录`,
        closeButton: true,
      });
      await loadUsage(range, false);
    } catch (reason) {
      const message = String(reason);
      setRebuildError(message);
      setError(message);
      toast.error("Codex 用量重建失败", {
        description:
          "旧数据库已在重建前尝试备份，可直接重试或从设置中恢复备份。",
        closeButton: true,
      });
    } finally {
      setRebuilding(false);
    }
  }, [loadUsage, range, rebuilding]);

  useEffect(() => {
    const shouldSyncSessions = !initialUsageLoadStarted.current;
    initialUsageLoadStarted.current = true;
    void loadUsage(range, shouldSyncSessions);
  }, [loadUsage, range]);

  const total = summary?.realTotalTokens ?? 0;
  const input = summary?.totalInputTokens ?? 0;
  const output = summary?.totalOutputTokens ?? 0;
  const cache =
    (summary?.totalCacheCreationTokens ?? 0) +
    (summary?.totalCacheReadTokens ?? 0);
  const chartTrends = trends.map((item) => ({
    ...item,
    totalTokens:
      item.totalInputTokens +
      item.totalOutputTokens +
      item.totalCacheCreationTokens +
      item.totalCacheReadTokens,
  }));
  // Denominator spans *every* model, not just the rendered ones, so a bar reads
  // "this model's share of all token use". Summing only the top 3 would force
  // them to 100% and overstate each one.
  const modelTotal = Math.max(
    models.reduce((sum, item) => sum + item.totalTokens, 0),
    1,
  );
  const topModels = topModelsByTokens(models);
  const displayTotal = total;
  const peak = Math.max(...chartTrends.map((item) => item.totalTokens), 0);
  const spectrum = ["#36c5d9", "#53d7c2", "#ffb84d", "#ff7e57", "#e85d9e"];
  const rebuildBackupName = rebuildResult?.backupPath
    ? rebuildResult.backupPath.split(/[\\/]/).pop()
    : null;
  return (
    <section className="usage-surface usage-spectrum">
      <div className="usage-heading">
        <div>
          <span className="eyebrow">本机统计</span>
          <h1>词元消耗</h1>
          <p>
            {syncing
              ? "正在同步本机会话记录…"
              : `${syncNote}，所有数据仅保存在这台电脑。`}
          </p>
        </div>
        <div className="usage-toolbar">
          <button
            className="usage-rebuild"
            onClick={() => setRebuildDialogOpen(true)}
            disabled={!runningInTauri || syncing || rebuilding}
          >
            <RotateCcw size={13} className={rebuilding ? "spin" : ""} />
            {rebuilding ? "重建中" : "重建用量"}
          </button>
          <button
            className="usage-refresh"
            onClick={() => void loadUsage(range, true)}
            disabled={!runningInTauri || syncing || rebuilding}
            aria-label="同步词元记录"
          >
            <RefreshCw size={14} className={syncing ? "spin" : ""} />
          </button>
          <div className="range-segment">
            {(
              [
                ["today", "今日"],
                ["7d", "7 天"],
                ["30d", "30 天"],
              ] as const
            ).map(([id, label]) => (
              <button
                key={id}
                className={range === id ? "is-active" : ""}
                onClick={() => setRange(id)}
                disabled={rebuilding}
                aria-pressed={range === id}
              >
                {label}
              </button>
            ))}
          </div>
        </div>
      </div>
      {rebuildResult && (
        <div className="usage-rebuild-result is-success" role="status">
          <ShieldCheck size={16} />
          <div>
            <strong>重建完成</strong>
            <span>
              扫描 {rebuildResult.filesScanned} 个文件，导入{" "}
              {rebuildResult.imported} 条，跳过 {rebuildResult.skipped} 条
              {rebuildResult.suspectedDuplicates
                ? `，识别 ${rebuildResult.suspectedDuplicates} 条疑似重复记录`
                : ""}
              {rebuildResult.errors.length
                ? `，另有 ${rebuildResult.errors.length} 项未能导入`
                : "。"}
            </span>
            <small title={rebuildResult.backupPath ?? undefined}>
              <DatabaseBackup size={12} />
              {rebuildBackupName
                ? `重建前备份：${rebuildBackupName}`
                : "首次运行尚无旧数据库，无需创建备份"}
            </small>
          </div>
        </div>
      )}
      {rebuildError && (
        <div className="usage-rebuild-result is-error" role="alert">
          <CircleAlert size={16} />
          <div>
            <strong>重建未完成</strong>
            <span>{rebuildError}</span>
            <button onClick={() => setRebuildDialogOpen(true)}>重新尝试</button>
          </div>
        </div>
      )}
      {error && (
        <div className="inline-error">
          <CircleAlert size={15} /> 词元统计暂时不可用：{error}
        </div>
      )}
      <article className="usage-spectrum-panel">
        <section className="usage-spectrum-summary">
          <span>
            {range === "today"
              ? "今日累计"
              : range === "7d"
                ? "7 天累计"
                : "30 天累计"}
            {rangeLoading ? " · 正在更新" : ""}
          </span>
          <strong>{displayTotal.toLocaleString("zh-CN")}</strong>
          <small>词元</small>
          <dl>
            <div>
              <dt>输入词元</dt>
              <dd className="is-input">{input.toLocaleString("zh-CN")}</dd>
            </div>
            <div>
              <dt>输出词元</dt>
              <dd className="is-output">{output.toLocaleString("zh-CN")}</dd>
            </div>
            <div>
              <dt>缓存词元</dt>
              <dd className="is-success">{cache.toLocaleString("zh-CN")}</dd>
            </div>
          </dl>
        </section>
        <section className="usage-spectrum-trend">
          <header>
            <b>每日消耗光谱</b>
            <span>
              峰值 {peak.toLocaleString("zh-CN")} ·{" "}
              {summary?.totalRequests ?? 0} 次请求 ·{" "}
              {summary
                ? `${Math.round(summary.successRate * 10) / 10}% 成功`
                : "--"}
            </span>
          </header>
          <div className="usage-spectrum-chart" aria-label="每日词元消耗光谱">
            {trends.length ? (
              <ResponsiveContainer
                width="100%"
                height="100%"
                initialDimension={{ width: 760, height: 190 }}
              >
                <BarChart
                  data={chartTrends}
                  margin={{ top: 12, right: 16, bottom: 0, left: 0 }}
                >
                  <CartesianGrid vertical={false} stroke="#e8edf0" />
                  <XAxis
                    dataKey="date"
                    axisLine={false}
                    tickLine={false}
                    minTickGap={24}
                    tick={{ fill: "#69737d", fontSize: 9 }}
                    tickFormatter={(value: string) => value.slice(5, 10)}
                  />
                  <YAxis hide domain={[0, "dataMax"]} />
                  <Tooltip
                    cursor={{ fill: "#eef5f6" }}
                    contentStyle={{
                      border: "1px solid #dfe6e9",
                      borderRadius: 8,
                      background: "#ffffff",
                      color: "#20272d",
                      fontSize: 11,
                      boxShadow: "0 4px 8px rgba(31, 43, 51, 0.1)",
                    }}
                    labelStyle={{ color: "#69737d", marginBottom: 4 }}
                    formatter={(value) => [
                      Number(value).toLocaleString("zh-CN"),
                      "词元",
                    ]}
                  />
                  <Bar
                    dataKey="totalTokens"
                    radius={[4, 4, 1, 1]}
                    maxBarSize={18}
                  >
                    {chartTrends.map((item, index) => (
                      <Cell
                        key={item.date}
                        fill={
                          spectrum[
                            Math.min(
                              spectrum.length - 1,
                              Math.floor(
                                (index / Math.max(1, trends.length - 1)) *
                                  spectrum.length,
                              ),
                            )
                          ]
                        }
                      />
                    ))}
                  </Bar>
                </BarChart>
              </ResponsiveContainer>
            ) : (
              <div className="chart-empty">暂无趋势数据</div>
            )}
          </div>
        </section>
        <section className="usage-spectrum-models" aria-label="模型词元分布">
          {topModels.length ? (
            topModels.map((item, index) => {
              const ratio = Math.round((item.totalTokens / modelTotal) * 100);
              const color = spectrum[(index * 2) % spectrum.length];
              return (
                <div className="usage-spectrum-model" key={item.model}>
                  <header>
                    <code title={item.model}>{item.model}</code>
                    <strong style={{ color }}>{ratio}%</strong>
                  </header>
                  <span>{item.totalTokens.toLocaleString("zh-CN")} 词元</span>
                  <i>
                    <u
                      style={{
                        width: `${Math.max(3, ratio)}%`,
                        background: color,
                        boxShadow: `0 0 8px ${color}80`,
                      }}
                    />
                  </i>
                </div>
              );
            })
          ) : (
            <p className="muted-copy">暂无模型统计。</p>
          )}
        </section>
      </article>
      <ConfirmDialog
        isOpen={rebuildDialogOpen}
        title="重建 Codex 用量？"
        message={
          "Chimera++ 会先备份当前数据库，再清理仅来自 Codex 会话的用量数据并重新扫描。\n\n代理记录和其他本地数据不会被删除。重建期间请勿退出应用。"
        }
        confirmText="备份并重建"
        cancelText="取消"
        variant="destructive"
        onConfirm={() => void rebuildUsage()}
        onCancel={() => setRebuildDialogOpen(false)}
      />
    </section>
  );
}
