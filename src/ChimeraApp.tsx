import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Activity,
  Check,
  ChevronDown,
  CircleAlert,
  Command,
  Download,
  Eye,
  EyeOff,
  LoaderCircle,
  FolderOpen,
  MoreHorizontal,
  Paintbrush,
  Plus,
  RefreshCw,
  Settings2,
  ShieldCheck,
  Trash2,
  Wrench,
  X,
  Zap,
} from "lucide-react";
import { toast } from "sonner";
import type { Provider } from "@/types";
import { providersApi } from "@/lib/api/providers";
import { settingsApi } from "@/lib/api/settings";
import { vscodeApi } from "@/lib/api/vscode";
import { usageApi } from "@/lib/api/usage";
import { useUpdate } from "@/contexts/UpdateContext";
import type { RequestLog } from "@/types/usage";
import type { Settings } from "@/types";
import { fetchModelsForConfig, type FetchedModel } from "@/lib/api/model-fetch";
import { getCodexCustomTemplate } from "@/config/codexTemplates";
import {
  extractCodexBaseUrl,
  extractCodexModelName,
  setCodexBaseUrl,
  setCodexModelName,
} from "@/utils/providerConfigUtils";
import { generateUUID } from "@/utils/uuid";
import {
  formatDuration,
  formatVersion,
  loadOperationRecords,
  resolveCurrentProvider,
  saveOperationRecords,
  type ConnectionState,
  type OperationRecord,
} from "./chimeraUtils";
import "./chimera.css";

function useEscapeClose(onClose: () => void, enabled = true) {
  useEffect(() => {
    if (!enabled) return;
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [enabled, onClose]);
}

type View = "providers" | "runtime" | "activity" | "appearance" | "settings";
type RuntimeStatus = {
  installed: boolean;
  version?: string | null;
  installMode?: string | null;
  installPath?: string | null;
  canRepair: boolean;
  canRollback: boolean;
  canUninstall: boolean;
};
type ReleaseStatus = {
  currentVersion?: string | null;
  latestVersion: string;
  updateAvailable: boolean;
  installMode: string;
  sizeBytes: number;
  source: string;
};
type Capability = { id: string; enabledByDefault: boolean };
type ProductCapabilities = { capabilities: Capability[] };
type Diagnostic = { name: string; result: string };
type DownloadProgress = { downloaded: number; total: number };
type CatalogSkin = {
  id: string;
  name: string;
  description?: string;
  version: string;
  author?: string;
  preview: string;
  installed: boolean;
  applied: boolean;
};

const nav: Array<[View, string, typeof Command]> = [
  ["providers", "供应商", Command],
  ["runtime", "Codex 运行时", Download],
  ["activity", "连接记录", Activity],
  ["appearance", "外观", Paintbrush],
  ["settings", "设置", Settings2],
];

const runtimeText = (mode?: string | null) =>
  mode === "standard" ? "稳定版" : "免安装版";

function providerDraft(provider?: Provider | null) {
  const template = getCodexCustomTemplate();
  const config = String(provider?.settingsConfig?.config ?? template.config);
  const auth = (provider?.settingsConfig?.auth ?? template.auth) as Record<
    string,
    unknown
  >;
  return {
    id: provider?.id ?? generateUUID(),
    name: provider?.name ?? "",
    websiteUrl: provider?.websiteUrl ?? "",
    notes: provider?.notes ?? "",
    baseUrl: extractCodexBaseUrl(config) ?? "",
    apiKey: String(auth.OPENAI_API_KEY ?? auth.api_key ?? ""),
    model: extractCodexModelName(config) ?? "",
    config,
    auth,
    original: provider ?? null,
  };
}

export default function ChimeraApp() {
  const { hasUpdate, updateInfo, isDismissed, dismissUpdate } = useUpdate();
  const [view, setView] = useState<View>("providers");
  const [providers, setProviders] = useState<Provider[]>([]);
  const [currentId, setCurrentId] = useState("");
  const [currentSource, setCurrentSource] = useState<
    "live" | "stored" | "external" | "none"
  >("none");
  const [loading, setLoading] = useState(true);
  const [runtime, setRuntime] = useState<RuntimeStatus | null>(null);
  const [release, setRelease] = useState<ReleaseStatus | null>(null);
  const [editor, setEditor] = useState<ReturnType<typeof providerDraft> | null>(
    null,
  );
  const [models, setModels] = useState<FetchedModel[] | null>(null);
  const [fetchingModels, setFetchingModels] = useState(false);
  const [showKey, setShowKey] = useState(false);
  const [pendingAction, setPendingAction] = useState<
    "update" | "repair" | "rollback" | "uninstall" | null
  >(null);
  const [skinEnabled, setSkinEnabled] = useState(false);
  const [activity, setActivity] = useState<OperationRecord[]>(() =>
    loadOperationRecords(),
  );
  const [requestLogs, setRequestLogs] = useState<RequestLog[]>([]);
  const [connection, setConnection] = useState<ConnectionState>({
    kind: "unknown",
    message: "尚未验证连接",
  });
  const [diagnostics, setDiagnostics] = useState<Diagnostic[] | null>(null);
  const [downloadProgress, setDownloadProgress] =
    useState<DownloadProgress | null>(null);
  const [installingAppUpdate, setInstallingAppUpdate] = useState(false);
  const [pendingProviderDelete, setPendingProviderDelete] =
    useState<Provider | null>(null);

  const loadProviders = async () => {
    try {
      const [all, stored] = await Promise.all([
        providersApi.getAll("codex"),
        providersApi.getCurrent("codex"),
      ]);
      const sorted = Object.values(all).sort(
        (a, b) => (a.sortIndex ?? 0) - (b.sortIndex ?? 0),
      );
      let live: unknown = null;
      let liveReadSucceeded = false;
      try {
        live = await vscodeApi.getLiveProviderSettings("codex");
        liveReadSucceeded = true;
      } catch {
        // The stored selection remains useful when Codex has not created its config yet.
      }
      const resolution = resolveCurrentProvider(
        sorted,
        stored,
        live,
        liveReadSucceeded,
      );
      setProviders(sorted);
      setCurrentId(resolution.provider?.id ?? "");
      setCurrentSource(resolution.source);
    } catch (error) {
      toast.error("无法读取 Codex 供应商", { description: String(error) });
    } finally {
      setLoading(false);
    }
  };

  const loadRuntime = async () => {
    try {
      const status = await invoke<RuntimeStatus>("get_codex_runtime_status");
      setRuntime(status);
    } catch (error) {
      setRuntime(null);
      if (view === "runtime")
        toast.error("无法读取 Codex 运行时状态", {
          description: String(error),
        });
    }
  };

  useEffect(() => {
    void loadProviders();
    void loadRuntime();
    void invoke<ProductCapabilities>("get_product_capabilities")
      .then((value) =>
        setSkinEnabled(
          value.capabilities.some(
            (item) => item.id === "codex_themes" && item.enabledByDefault,
          ),
        ),
      )
      .catch(() => setSkinEnabled(false));
    void usageApi
      .getRequestLogs({ appType: "codex" }, 0, 50)
      .then((result) => setRequestLogs(result.data))
      .catch(() => setRequestLogs([]));
    const unlisten = listen<DownloadProgress>(
      "codex-runtime-download-progress",
      (event) => {
        setDownloadProgress(event.payload);
      },
    );
    return () => {
      void unlisten.then((dispose) => dispose());
    };
  }, []);

  const note = (
    action: string,
    result: OperationRecord["result"] = "success",
    detail?: string,
    provider = "Codex",
    durationMs?: number,
  ) => {
    setActivity((items) =>
      saveOperationRecords([
        {
          id: generateUUID(),
          timestamp: Date.now(),
          provider,
          action,
          result,
          detail,
          durationMs,
        },
        ...items,
      ]),
    );
  };

  const switchProvider = async (id: string) => {
    const started = performance.now();
    try {
      await providersApi.switch(id, "codex");
      await loadProviders();
      const provider = providers.find((item) => item.id === id);
      note(
        "切换供应商",
        "success",
        "配置已写入 Codex",
        provider?.name ?? id,
        performance.now() - started,
      );
      toast.success("已应用到 Codex");
    } catch (error) {
      note(
        "切换供应商",
        "error",
        String(error),
        providers.find((item) => item.id === id)?.name ?? id,
        performance.now() - started,
      );
      toast.error("切换失败", { description: String(error) });
    }
  };

  const testConnection = async (baseUrl: string, providerName = "Codex") => {
    const started = performance.now();
    setConnection({ kind: "checking", message: "正在测试 API 地址" });
    try {
      const [result] = await vscodeApi.testApiEndpoints([baseUrl], {
        timeoutSecs: 12,
      });
      if (!result || result.latency == null)
        throw new Error(result?.error || "服务未响应");
      setConnection({
        kind: "connected",
        message: `${result.latency}ms`,
        modelCount: models?.length ?? 0,
      });
      note(
        "连接测试",
        "success",
        `${result.latency}ms`,
        providerName,
        performance.now() - started,
      );
      toast.success("连接可用", {
        description: `响应时间 ${result.latency}ms`,
      });
      return true;
    } catch (error) {
      setConnection({ kind: "error", message: String(error) });
      note(
        "连接测试",
        "error",
        String(error),
        providerName,
        performance.now() - started,
      );
      toast.error("连接测试失败", { description: String(error) });
      return false;
    }
  };

  const saveProvider = async () => {
    if (!editor) return;
    if (
      !editor.name.trim() ||
      !editor.baseUrl.trim() ||
      !editor.apiKey.trim()
    ) {
      toast.error("请填写供应商名称、API 请求地址和 API Key");
      return;
    }
    const config = setCodexModelName(
      setCodexBaseUrl(editor.config, editor.baseUrl),
      editor.model,
    );
    const provider: Provider = {
      id: editor.id,
      name: editor.name.trim(),
      websiteUrl: editor.websiteUrl.trim() || undefined,
      notes: editor.notes.trim() || undefined,
      category: "custom",
      settingsConfig: {
        ...editor.original?.settingsConfig,
        auth: { ...editor.auth, OPENAI_API_KEY: editor.apiKey.trim() },
        config,
        ...(models
          ? {
              modelCatalog: {
                models: models.map((model) => ({
                  id: model.id,
                  name: model.id,
                })),
              },
            }
          : {}),
      },
    };
    try {
      if (editor.original)
        await providersApi.update(provider, "codex", editor.original.id);
      else await providersApi.add(provider, "codex", false);
      await providersApi.switch(provider.id, "codex");
      await loadProviders();
      setEditor(null);
      note("保存并应用供应商", "success", undefined, provider.name);
      toast.success("供应商已保存");
    } catch (error) {
      toast.error("保存失败", { description: String(error) });
    }
  };

  const fetchModels = async () => {
    if (!editor?.baseUrl.trim() || !editor.apiKey.trim()) {
      toast.error("请先填写 API 请求地址和 API Key");
      return;
    }
    setFetchingModels(true);
    try {
      const result = await fetchModelsForConfig(editor.baseUrl, editor.apiKey);
      setModels(result);
      note(
        "获取模型",
        "success",
        `获取到 ${result.length} 个模型`,
        editor.name || "未命名供应商",
      );
      toast.success(`已获取 ${result.length} 个模型`);
    } catch (error) {
      note("获取模型", "error", String(error), editor?.name || "未命名供应商");
      toast.error("获取模型失败，可手动输入模型名称", {
        description: String(error),
      });
      setModels([]);
    } finally {
      setFetchingModels(false);
    }
  };

  const checkRuntime = async () => {
    try {
      const result = await invoke<ReleaseStatus>("check_codex_runtime_update", {
        source: null,
        installMode: null,
      });
      setRelease(result);
      note(
        "检查 Codex 更新",
        "success",
        result.updateAvailable
          ? `发现 ${result.latestVersion}`
          : "已是最新版本",
      );
      toast.success(
        result.updateAvailable ? "发现新版本" : "Codex 已是最新版本",
      );
    } catch (error) {
      toast.error("检查更新失败", { description: String(error) });
    }
  };

  const diagnose = async () => {
    try {
      const result = await invoke<Diagnostic[]>("diagnose_codex_runtime");
      setDiagnostics(result);
      note("运行诊断", "success", `${result.length} 项`);
    } catch (error) {
      note("运行诊断", "error", String(error));
      toast.error("诊断失败", { description: String(error) });
    }
  };

  const runRuntimeAction = async () => {
    if (!pendingAction) return;
    const action = pendingAction;
    const started = performance.now();
    setDownloadProgress(
      action === "update" || action === "repair"
        ? { downloaded: 0, total: release?.sizeBytes ?? 0 }
        : null,
    );
    try {
      if (action === "update") {
        await invoke("apply_codex_runtime_update", {
          expectedVersion: release?.latestVersion ?? null,
          source: null,
          installMode: null,
          confirm: true,
        });
      } else if (action === "repair") {
        await invoke("repair_codex_runtime", {
          source: null,
          installMode: null,
          confirm: true,
        });
      } else if (action === "rollback") {
        await invoke("rollback_codex_runtime", { confirm: true });
      } else {
        await invoke("uninstall_codex_runtime", { confirm: true });
      }
      note(
        `Codex ${action === "update" ? "更新" : action === "repair" ? "修复" : action === "rollback" ? "回滚" : "卸载"}`,
        "success",
        undefined,
        "Codex",
        performance.now() - started,
      );
      toast.success("操作已完成");
      await loadRuntime();
    } catch (error) {
      note(
        `Codex ${action}`,
        "error",
        String(error),
        "Codex",
        performance.now() - started,
      );
      toast.error("操作失败", { description: String(error) });
    } finally {
      setPendingAction(null);
      setDownloadProgress(null);
    }
  };

  return (
    <div className="chimera-shell">
      <aside className="chimera-sidebar">
        <div className="chimera-brand">
          <span>
            <Command size={16} />
          </span>
          <strong>Chimera++</strong>
        </div>
        <p className="chimera-kicker">CODEX WORKSPACE</p>
        <nav>
          {nav.map(([id, label, Icon]) => (
            <button
              key={id}
              className={view === id ? "is-active" : ""}
              onClick={() => setView(id)}
            >
              <Icon size={17} />
              {label}
            </button>
          ))}
        </nav>
        <div className="chimera-sidebar-footer">
          <span className="status-dot" /> 全部服务正常
          <br />
          <small>最后检测 刚刚</small>
        </div>
        <div className="workspace-identity">
          <b>H</b>
          <span>
            本机工作区<small>Chimera++ 2.0</small>
          </span>
        </div>
      </aside>
      <main className="chimera-main">
        <header className="chimera-titlebar" data-tauri-drag-region>
          <div data-tauri-drag-region>
            <h1>
              {view === "providers"
                ? "供应商控制台"
                : nav.find((item) => item[0] === view)?.[1]}
            </h1>
            <p>
              {view === "providers"
                ? "切换、配置并验证当前 API 连接"
                : view === "runtime"
                  ? "安装、更新与修复本机 Codex"
                  : view === "appearance"
                    ? "管理 Codex 客户端皮肤"
                    : view === "settings"
                      ? "应用行为、数据位置与更新策略"
                      : "查看供应商切换、连接测试与模型同步历史"}
            </p>
          </div>
          <WindowControls />
        </header>
        {hasUpdate && updateInfo && !isDismissed && (
          <div className="app-update-notice" role="status">
            <RefreshCw size={16} />
            <div>
              <b>Chimera++ {updateInfo.availableVersion} 可用</b>
              <span>已通过签名验证，安装后将自动重启。</span>
            </div>
            <button onClick={dismissUpdate} disabled={installingAppUpdate}>
              稍后
            </button>
            <button
              className="primary"
              disabled={installingAppUpdate}
              onClick={() => {
                setInstallingAppUpdate(true);
                void settingsApi.installUpdateAndRestart().catch((reason) => {
                  setInstallingAppUpdate(false);
                  toast.error("应用更新失败", { description: String(reason) });
                });
              }}
            >
              {installingAppUpdate ? "正在更新…" : "立即更新"}
            </button>
          </div>
        )}
        {view === "providers" && (
          <ProvidersView
            providers={providers}
            currentId={currentId}
            currentSource={currentSource}
            connection={connection}
            loading={loading}
            runtime={runtime}
            activity={activity}
            onSwitch={switchProvider}
            onEdit={(provider) => {
              setModels(null);
              setEditor(providerDraft(provider));
            }}
            onAdd={() => {
              setModels(null);
              setEditor(providerDraft());
            }}
            onTest={testConnection}
            onCheckRuntime={checkRuntime}
            onDiagnose={diagnose}
          />
        )}
        {view === "runtime" && (
          <RuntimeView
            runtime={runtime}
            release={release}
            progress={downloadProgress}
            onCheck={checkRuntime}
            onDiagnose={diagnose}
            onAction={setPendingAction}
          />
        )}
        {view === "activity" && (
          <ActivityView entries={activity} requests={requestLogs} />
        )}
        {view === "appearance" && <AppearanceView enabled={skinEnabled} />}
        {view === "settings" && <SettingsView onCheck={checkRuntime} />}
      </main>
      {editor && (
        <ProviderEditor
          editor={editor}
          setEditor={setEditor}
          showKey={showKey}
          setShowKey={setShowKey}
          models={models}
          fetchingModels={fetchingModels}
          onFetchModels={fetchModels}
          onSave={saveProvider}
          escapeDisabled={Boolean(pendingProviderDelete)}
          onDelete={() => {
            if (editor.original) setPendingProviderDelete(editor.original);
          }}
        />
      )}
      {pendingProviderDelete && (
        <ConfirmProviderDelete
          provider={pendingProviderDelete}
          onCancel={() => setPendingProviderDelete(null)}
          onConfirm={async () => {
            try {
              await providersApi.delete(pendingProviderDelete.id, "codex");
              await loadProviders();
              setPendingProviderDelete(null);
              setEditor(null);
              toast.success("供应商已删除");
            } catch (error) {
              toast.error("删除失败", { description: String(error) });
            }
          }}
        />
      )}
      {pendingAction && (
        <ConfirmOperation
          action={pendingAction}
          onCancel={() => setPendingAction(null)}
          onConfirm={runRuntimeAction}
        />
      )}
      {diagnostics && (
        <DiagnosticsDialog
          diagnostics={diagnostics}
          onClose={() => setDiagnostics(null)}
        />
      )}
    </div>
  );
}

function ProvidersView({
  providers,
  currentId,
  currentSource,
  connection,
  loading,
  runtime,
  activity,
  onSwitch,
  onEdit,
  onAdd,
  onTest,
  onCheckRuntime,
  onDiagnose,
}: {
  providers: Provider[];
  currentId: string;
  currentSource: "live" | "stored" | "external" | "none";
  connection: ConnectionState;
  loading: boolean;
  runtime: RuntimeStatus | null;
  activity: OperationRecord[];
  onSwitch: (id: string) => void;
  onEdit: (provider: Provider) => void;
  onAdd: () => void;
  onTest: (url: string, name?: string) => Promise<boolean>;
  onCheckRuntime: () => void;
  onDiagnose: () => void;
}) {
  if (loading) return <Empty label="正在读取供应商…" />;
  if (!providers.length) return <Onboarding onAdd={onAdd} />;
  const current =
    providers.find((provider) => provider.id === currentId) ?? null;
  if (!current)
    return (
      <section className="provider-console">
        <div className="connection-banner is-warning">
          <CircleAlert size={18} />
          <div>
            <b>检测到外部 Codex 配置</b>
            <span>
              当前配置不属于 Chimera++
              中已保存的供应商；请选择一个供应商应用，或添加现有配置。
            </span>
          </div>
          <em>未接管</em>
        </div>
        <div className="console-heading">
          <h2>已保存的供应商</h2>
          <button className="primary" onClick={onAdd}>
            <Plus size={15} /> 添加供应商
          </button>
        </div>
        <div className="provider-list">
          {providers.map((provider) => (
            <article className="provider-card" key={provider.id}>
              <span className="provider-monogram">
                {provider.name.slice(0, 1).toUpperCase()}
              </span>
              <div className="provider-copy">
                <b>{provider.name}</b>
                <code>
                  {extractCodexBaseUrl(
                    String(provider.settingsConfig?.config ?? ""),
                  ) || "未配置 URL"}
                </code>
              </div>
              <div className="provider-actions">
                <button onClick={() => onEdit(provider)}>编辑</button>
                <button className="dark" onClick={() => onSwitch(provider.id)}>
                  应用
                </button>
              </div>
            </article>
          ))}
        </div>
      </section>
    );
  const endpoint =
    extractCodexBaseUrl(String(current.settingsConfig?.config ?? "")) ||
    "未配置请求地址";
  const model =
    extractCodexModelName(String(current.settingsConfig?.config ?? "")) ||
    "未设置";
  const cards = providers.slice(0, 3);
  const connectionLabel =
    connection.kind === "connected"
      ? `已验证 · ${connection.message}`
      : connection.kind === "checking"
        ? "验证中"
        : connection.kind === "error"
          ? "验证失败"
          : currentSource === "live"
            ? "配置已识别"
            : "等待验证";
  return (
    <section className="provider-console">
      <div
        className={`connection-banner ${connection.kind === "error" ? "is-warning" : ""}`}
      >
        <Zap size={18} />
        <div>
          <b>当前正在使用 {current.name}</b>
          <span>
            {currentSource === "live"
              ? "已从 Codex 实时配置识别"
              : "根据 Chimera++ 保存记录识别"}
          </span>
        </div>
        <em>{connectionLabel}</em>
      </div>
      <div className="console-layout">
        <div className="console-main">
          <div className="console-heading">
            <h2>快速切换</h2>
            <button className="link-button" onClick={() => onEdit(current)}>
              管理供应商 <span>→</span>
            </button>
          </div>
          <div className="quick-provider-grid">
            {cards.map((provider) => {
              const active = provider.id === current.id;
              return (
                <button
                  key={provider.id}
                  className={`quick-provider ${active ? "selected" : ""}`}
                  onClick={() => !active && onSwitch(provider.id)}
                >
                  <span className="quick-provider-mark">
                    {provider.name.slice(0, 1).toUpperCase()}
                  </span>
                  <b title={provider.name}>{provider.name}</b>
                  <em>{active ? "当前" : "可切换"}</em>
                  <small
                    title={
                      extractCodexModelName(
                        String(provider.settingsConfig?.config ?? ""),
                      ) || "未配置模型"
                    }
                  >
                    {extractCodexModelName(
                      String(provider.settingsConfig?.config ?? ""),
                    ) || "未配置模型"}
                  </small>
                </button>
              );
            })}
            <button className="quick-provider add-provider" onClick={onAdd}>
              <Plus size={16} /> 添加供应商
            </button>
          </div>
          <article className="provider-workbench">
            <header>
              <div>
                <h2>{current.name}</h2>
                <p>Codex 兼容接口 · 模型由供应商 API 获取</p>
              </div>
              <button className="preset-badge" onClick={() => onEdit(current)}>
                编辑
              </button>
            </header>
            <label>
              接口地址
              <input value={endpoint} readOnly title={endpoint} />
            </label>
            <label>
              API 密钥
              <div className="readonly-secret">
                <input value="••••••••••••••••••" readOnly />
                <button onClick={() => onEdit(current)}>编辑</button>
              </div>
            </label>
            <label>
              默认模型
              <div className="readonly-model">
                <input value={model} readOnly title={model} />
                <button onClick={() => onEdit(current)}>获取模型</button>
              </div>
            </label>
            <footer>
              <button
                className="secondary"
                onClick={() => void onTest(endpoint, current.name)}
                disabled={!endpoint}
              >
                测试连接
              </button>
              <button className="primary" onClick={() => onEdit(current)}>
                编辑配置
              </button>
            </footer>
          </article>
        </div>
        <aside className="codex-summary">
          <div className="summary-title">
            <h2>Codex 运行时</h2>
            <button aria-label="运行时诊断" onClick={onDiagnose}>
              <MoreHorizontal size={18} />
            </button>
          </div>
          <div className="runtime-version">
            <b title={runtime?.version ?? undefined}>
              {formatVersion(runtime?.version)}
            </b>
            <em>{runtime?.installed ? "已安装" : "未安装"}</em>
            <span>
              {runtime?.installed
                ? `${runtimeText(runtime.installMode)} · 路径已识别`
                : "未检测到可用安装"}
            </span>
          </div>
          <ul className="runtime-facts">
            <li>
              <ShieldCheck size={16} />
              <span>
                <b>运行时检测</b>
                <small>
                  {runtime?.installed
                    ? "Codex App Manager 引擎已识别"
                    : "等待安装或重新检测"}
                </small>
              </span>
            </li>
            <li>
              <Check size={16} />
              <span>
                <b>安装位置</b>
                <small title={runtime?.installPath ?? undefined}>
                  {runtime?.installPath || "未检测到"}
                </small>
              </span>
            </li>
            <li>
              <Activity size={16} />
              <span>
                <b>回滚点</b>
                <small>
                  {runtime?.canRollback ? "可用" : "当前安装方式无可用副本"}
                </small>
              </span>
            </li>
          </ul>
          <div className="summary-actions">
            <button className="dark" onClick={onCheckRuntime}>
              <RefreshCw size={15} /> 检查更新
            </button>
            <button onClick={onDiagnose}>
              <Wrench size={15} /> 查看诊断
            </button>
          </div>
          <div className="summary-activity">
            <b>最近活动</b>
            {activity.slice(0, 2).map((item) => (
              <span key={item.id}>
                {new Date(item.timestamp).toLocaleTimeString("zh-CN", {
                  hour: "2-digit",
                  minute: "2-digit",
                })}{" "}
                · {item.action}
              </span>
            ))}
            {!activity.length && <span>暂无操作记录</span>}
          </div>
        </aside>
      </div>
    </section>
  );
}

function ProviderEditor({
  editor,
  setEditor,
  showKey,
  setShowKey,
  models,
  fetchingModels,
  onFetchModels,
  onSave,
  onDelete,
  escapeDisabled,
}: {
  editor: ReturnType<typeof providerDraft>;
  setEditor: (value: ReturnType<typeof providerDraft> | null) => void;
  showKey: boolean;
  setShowKey: (value: boolean) => void;
  models: FetchedModel[] | null;
  fetchingModels: boolean;
  onFetchModels: () => void;
  onSave: () => void;
  onDelete: () => void;
  escapeDisabled: boolean;
}) {
  useEscapeClose(() => setEditor(null), !escapeDisabled);
  const patch = (key: string, value: string) =>
    setEditor({ ...editor, [key]: value });
  return (
    <div className="modal-backdrop">
      <section
        className="provider-editor"
        role="dialog"
        aria-modal="true"
        aria-labelledby="provider-editor-title"
      >
        <header>
          <button
            className="icon-button"
            aria-label="关闭供应商编辑器"
            onClick={() => setEditor(null)}
          >
            <X size={18} />
          </button>
          <div>
            <h2 id="provider-editor-title">
              {editor.original ? "编辑供应商" : "添加供应商"}
            </h2>
            <p>仅写入 Codex 的供应商配置。</p>
          </div>
        </header>
        <div className="editor-form">
          <Field
            label="供应商名称"
            value={editor.name}
            onChange={(value) => patch("name", value)}
            placeholder="例如 Chimera Hub"
          />
          <Field
            label="官网链接"
            value={editor.websiteUrl}
            onChange={(value) => patch("websiteUrl", value)}
            placeholder="https://example.com"
          />
          <Field
            label="API 请求地址"
            value={editor.baseUrl}
            onChange={(value) => patch("baseUrl", value)}
            placeholder="https://api.example.com/v1"
            hint="预设和自定义供应商都可编辑 URL。"
          />
          <label>
            API Key
            <div className="password-field">
              <input
                type={showKey ? "text" : "password"}
                value={editor.apiKey}
                onChange={(event) => patch("apiKey", event.target.value)}
                placeholder="粘贴 API Key"
              />
              <button
                aria-label={showKey ? "隐藏 API Key" : "显示 API Key"}
                onClick={() => setShowKey(!showKey)}
              >
                {showKey ? <EyeOff size={16} /> : <Eye size={16} />}
              </button>
            </div>
          </label>
          <label>
            默认模型
            <div className="model-input">
              <input
                value={editor.model}
                onChange={(event) => patch("model", event.target.value)}
                placeholder="先获取模型列表，或手动输入"
              />
              <button onClick={onFetchModels} disabled={fetchingModels}>
                {fetchingModels ? (
                  <LoaderCircle className="spin" size={15} />
                ) : (
                  <Download size={15} />
                )}{" "}
                获取模型
              </button>
            </div>
          </label>
          {models !== null && (
            <div className="model-results">
              {models.length ? (
                models.map((model) => (
                  <button
                    key={model.id}
                    className={editor.model === model.id ? "picked" : ""}
                    onClick={() => patch("model", model.id)}
                  >
                    {model.id}
                    {editor.model === model.id && <Check size={15} />}
                  </button>
                ))
              ) : (
                <p>
                  <CircleAlert size={15} />{" "}
                  未能获取模型列表，可保留手动输入的模型名称。
                </p>
              )}
            </div>
          )}
          <details>
            <summary>高级选项</summary>
            <p>
              默认采用 Codex Responses
              兼容配置。复杂协议和模型映射仅在需要时展开。
            </p>
          </details>
        </div>
        <footer>
          <button
            className="secondary"
            onClick={async () => {
              await onFetchModels();
            }}
          >
            测试连接
          </button>
          <div>
            {editor.original && (
              <button className="danger" onClick={onDelete}>
                <Trash2 size={15} /> 删除
              </button>
            )}
            <button className="primary" onClick={onSave}>
              保存并应用
            </button>
          </div>
        </footer>
      </section>
    </div>
  );
}

function RuntimeView({
  runtime,
  release,
  progress,
  onCheck,
  onDiagnose,
  onAction,
}: {
  runtime: RuntimeStatus | null;
  release: ReleaseStatus | null;
  progress: DownloadProgress | null;
  onCheck: () => void;
  onDiagnose: () => void;
  onAction: (value: "update" | "repair" | "rollback" | "uninstall") => void;
}) {
  const target = release?.latestVersion ?? runtime?.version ?? "等待检查";
  const percent = progress?.total
    ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100))
    : 0;
  return (
    <section className="runtime-update-layout">
      <article className="runtime-update-card">
        <h2>{release?.updateAvailable ? "发现可用更新" : "Codex 运行时"}</h2>
        <div className="version-compare">
          <div>
            <span>当前版本</span>
            <b title={runtime?.version ?? undefined}>
              {formatVersion(runtime?.version)}
            </b>
          </div>
          <div
            className={
              release?.updateAvailable
                ? "target-version available"
                : "target-version"
            }
          >
            <span>目标版本</span>
            <b title={target}>{target}</b>
          </div>
        </div>
        <dl className="update-details">
          <div>
            <dt>更新通道</dt>
            <dd>
              <Check size={15} />{" "}
              {release?.source === "mirror" ? "镜像" : "自动"}
            </dd>
          </div>
          <div>
            <dt>安装方式</dt>
            <dd>
              <Check size={15} />{" "}
              {release
                ? runtimeText(release.installMode)
                : runtimeText(runtime?.installMode)}
            </dd>
          </div>
          <div>
            <dt>安装状态</dt>
            <dd>
              {runtime?.installed ? (
                <>
                  <Check size={15} /> 已检测
                </>
              ) : (
                "未安装"
              )}
            </dd>
          </div>
          <div>
            <dt>下载大小</dt>
            <dd>
              {release?.sizeBytes
                ? `${(release.sizeBytes / 1024 / 1024).toFixed(1)} MB`
                : "检查后显示"}
            </dd>
          </div>
        </dl>
        <div className="update-progress" aria-live="polite">
          <span>
            {progress
              ? `正在下载 ${percent}%`
              : release?.updateAvailable
                ? "新版本可以下载安装"
                : release
                  ? "当前通道没有更高版本"
                  : "检查更新以获取最新版本"}
          </span>
          <i>
            <u style={{ width: `${percent}%` }} />
          </i>
        </div>
        <footer>
          <button onClick={onCheck} disabled={Boolean(progress)}>
            重新检查
          </button>
          {release?.updateAvailable && (
            <button
              className="primary"
              onClick={() => onAction("update")}
              disabled={Boolean(progress)}
            >
              下载并安装
            </button>
          )}
        </footer>
      </article>
      <aside className="runtime-diagnostics">
        <h2>修复与诊断</h2>
        <p>
          所有维护能力来自 Codex App Manager 引擎；操作前会二次确认，并保留
          `~/.codex` 用户数据。
        </p>
        <button onClick={onDiagnose}>
          查看诊断结果 <span>↗</span>
          <small>安装目录、版本、进程和启动状态</small>
        </button>
        <button
          onClick={() => onAction("rollback")}
          disabled={!runtime?.canRollback}
        >
          回滚上一版本 <span>↗</span>
          <small>仅免安装版且存在回滚点时可用</small>
        </button>
        <button
          onClick={() => onAction("repair")}
          disabled={!runtime?.canRepair}
        >
          重新安装并修复 <span>↗</span>
          <small>使用当前安装方式</small>
        </button>
        <button
          className="danger-line"
          onClick={() => onAction("uninstall")}
          disabled={!runtime?.canUninstall}
        >
          卸载 Codex
        </button>
      </aside>
    </section>
  );
}

function ActivityView({
  entries,
  requests,
}: {
  entries: OperationRecord[];
  requests: RequestLog[];
}) {
  const requestErrors = requests.filter(
    (item) => item.statusCode >= 400,
  ).length;
  const success = entries.filter((item) => item.result === "success").length;
  const errors =
    entries.filter((item) => item.result === "error").length + requestErrors;
  return (
    <section className="activity-dashboard">
      <div className="activity-metrics">
        <Metric
          label="API 请求"
          value={String(requests.length)}
          detail="CC Switch 代理记录"
        />
        <Metric
          label="本机操作"
          value={String(entries.length)}
          detail={`${success} 项成功`}
          success
        />
        <Metric label="异常记录" value={String(errors)} detail="需要处理" />
      </div>
      <article className="activity-table">
        <div className="activity-table-head">
          <span>时间</span>
          <span>供应商</span>
          <span>操作 / 模型</span>
          <span>结果</span>
        </div>
        {requests.map((entry) => (
          <div
            className="activity-table-row"
            key={entry.requestId}
            title={entry.errorMessage}
          >
            <span>
              {new Date(entry.createdAt).toLocaleString("zh-CN", {
                month: "2-digit",
                day: "2-digit",
                hour: "2-digit",
                minute: "2-digit",
              })}
            </span>
            <span>{entry.providerName || entry.providerId}</span>
            <span>
              {entry.model} ·{" "}
              {formatDuration(entry.durationMs ?? entry.latencyMs)}
            </span>
            <span className={entry.statusCode < 400 ? "ok" : "error-text"}>
              {entry.statusCode}
            </span>
          </div>
        ))}
        {entries.map((entry) => (
          <div
            className="activity-table-row"
            key={entry.id}
            title={entry.detail}
          >
            <span>
              {new Date(entry.timestamp).toLocaleString("zh-CN", {
                month: "2-digit",
                day: "2-digit",
                hour: "2-digit",
                minute: "2-digit",
              })}
            </span>
            <span>{entry.provider}</span>
            <span>
              {entry.action}
              {entry.durationMs != null
                ? ` · ${formatDuration(entry.durationMs)}`
                : ""}
            </span>
            <span className={entry.result === "success" ? "ok" : "error-text"}>
              {entry.result === "success"
                ? "成功"
                : entry.result === "error"
                  ? "失败"
                  : "已跳过"}
            </span>
          </div>
        ))}
        {!entries.length && !requests.length && (
          <Empty label="暂无记录。代理请求和 Chimera++ 操作会显示在这里。" />
        )}
      </article>
    </section>
  );
}

function AppearanceView({ enabled }: { enabled: boolean }) {
  const [skins, setSkins] = useState<CatalogSkin[]>([]);
  const [selectedId, setSelectedId] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState("");
  const load = async () => {
    try {
      setError("");
      const result = await invoke<CatalogSkin[]>("list_skin_catalog");
      setSkins(result);
      setSelectedId((id) =>
        id && result.some((item) => item.id === id)
          ? id
          : (result[0]?.id ?? ""),
      );
    } catch (reason) {
      setError(String(reason));
    }
  };
  useEffect(() => {
    if (enabled) void load();
  }, [enabled]);
  const selected = skins.find((item) => item.id === selectedId) ?? null;
  const run = async (
    label: string,
    command: string,
    args?: Record<string, unknown>,
  ) => {
    try {
      setBusy(label);
      await invoke(command, args);
      toast.success(`${label}完成`);
      await load();
    } catch (reason) {
      toast.error(`${label}失败`, { description: String(reason) });
    } finally {
      setBusy(null);
    }
  };
  const importLocal = async () => {
    const path = await settingsApi.openFileDialog();
    if (path) await run("导入皮肤", "import_skin_package", { path });
  };
  if (!enabled) return <Empty label="当前产品策略未启用 Codex 皮肤能力。" />;
  return (
    <section className="skin-layout">
      <aside className="skin-list">
        <div className="skin-tabs">
          <b>在线皮肤</b>
          <button onClick={() => void importLocal()} disabled={Boolean(busy)}>
            导入本地
          </button>
        </div>
        {skins.map((skin) => (
          <button
            key={skin.id}
            className={skin.id === selectedId ? "active" : ""}
            onClick={() => setSelectedId(skin.id)}
          >
            <img
              src={`https://skins.agentsmirror.com/${skin.preview.replace(/^\/+/, "")}`}
              alt=""
            />
            {skin.name}
            <small>
              {skin.applied
                ? "正在使用"
                : skin.installed
                  ? "已安装"
                  : `${skin.author || "Chimera"} · ${skin.version}`}
            </small>
          </button>
        ))}
        {!skins.length && !error && <Empty label="正在读取皮肤目录…" />}
        {error && (
          <Empty
            label={`皮肤目录读取失败：${error}`}
            action="重试"
            onAction={() => void load()}
          />
        )}
      </aside>
      <article className="skin-detail">
        {selected ? (
          <>
            <h2>{selected.name}</h2>
            <p>
              {selected.description ||
                `${selected.author || "Chimera"} · ${selected.version}`}
            </p>
            <div className="skin-preview skin-preview-image">
              <img
                src={`https://skins.agentsmirror.com/${selected.preview.replace(/^\/+/, "")}`}
                alt={`${selected.name} 预览`}
              />
            </div>
            <div className="skin-actions">
              {!selected.installed && (
                <button
                  className="dark"
                  onClick={() =>
                    void run("下载安装", "install_catalog_skin", {
                      skinId: selected.id,
                    })
                  }
                  disabled={Boolean(busy)}
                >
                  {busy === "下载安装" ? "正在下载…" : "下载安装"}
                </button>
              )}
              {selected.installed && (
                <button
                  className="dark"
                  onClick={() =>
                    void run("应用皮肤", "apply_skin_package", {
                      skinId: selected.id,
                    })
                  }
                  disabled={Boolean(busy) || selected.applied}
                >
                  {selected.applied ? "正在使用" : "应用"}
                </button>
              )}
              <button
                onClick={() =>
                  void run("试穿", "try_skin_package", { skinId: selected.id })
                }
                disabled={Boolean(busy) || !selected.installed}
              >
                试穿
              </button>
              <button
                onClick={() => void run("恢复默认", "restore_skin_package")}
                disabled={Boolean(busy)}
              >
                恢复默认
              </button>
            </div>
            <p className="integrity">
              <ShieldCheck size={16} /> 皮肤包经过 SHA256 校验，并由 Codex App
              Manager 主题引擎应用。
            </p>
          </>
        ) : (
          <Empty label="选择一个皮肤查看预览。" />
        )}
      </article>
    </section>
  );
}

function SettingsView({ onCheck }: { onCheck: () => void }) {
  const [section, setSection] = useState<"general" | "runtime" | "data">(
    "general",
  );
  const [settings, setSettings] = useState<Settings | null>(null);
  const [autoLaunch, setAutoLaunch] = useState<boolean | null>(null);
  const [configPath, setConfigPath] = useState("");
  useEffect(() => {
    void Promise.all([
      settingsApi.get(),
      settingsApi.getAutoLaunchStatus(),
      settingsApi.getAppConfigPath(),
    ])
      .then(([value, launch, path]) => {
        setSettings(value);
        setAutoLaunch(launch);
        setConfigPath(path);
      })
      .catch((reason) =>
        toast.error("无法读取设置", { description: String(reason) }),
      );
  }, []);
  const save = async (patch: Partial<Settings>) => {
    if (!settings) return;
    const next = { ...settings, ...patch };
    try {
      await settingsApi.save(next);
      setSettings(next);
      toast.success("设置已保存");
    } catch (reason) {
      toast.error("设置保存失败", { description: String(reason) });
    }
  };
  const toggleAutoLaunch = async () => {
    try {
      const value = await settingsApi.setAutoLaunch(!(autoLaunch ?? false));
      setAutoLaunch(value);
      toast.success(value ? "已开启开机启动" : "已关闭开机启动");
    } catch (reason) {
      toast.error("设置失败", { description: String(reason) });
    }
  };
  const pickPortable = async () => {
    const path = await settingsApi.pickDirectory(settings?.codexPortableRoot);
    if (path) await save({ codexPortableRoot: path });
  };
  const pickData = async () => {
    const path = await settingsApi.pickDirectory(configPath);
    if (!path) return;
    try {
      await settingsApi.setAppConfigDirOverride(path);
      setConfigPath(path);
      toast.success("数据目录已更新，重启后生效");
    } catch (reason) {
      toast.error("目录设置失败", { description: String(reason) });
    }
  };
  return (
    <section className="settings-layout">
      <aside>
        {[
          ["general", "常规"],
          ["runtime", "更新策略"],
          ["data", "数据与隐私"],
        ].map(([id, label]) => (
          <button
            key={id}
            className={section === id ? "active" : ""}
            onClick={() => setSection(id as typeof section)}
          >
            {label}
          </button>
        ))}
      </aside>
      <article className="panel settings-panel">
        {section === "general" && (
          <>
            <h2>常规</h2>
            <button
              className="setting-row"
              onClick={() => void toggleAutoLaunch()}
            >
              <div>
                <b>开机启动 Chimera++</b>
                <p>登录 Windows 后自动运行</p>
              </div>
              <span>
                {autoLaunch === null ? "读取中" : autoLaunch ? "开启" : "关闭"}
                <ChevronDown size={14} />
              </span>
            </button>
            <div className="setting-row">
              <div>
                <b>语言</b>
                <p>当前版本的客户界面语言</p>
              </div>
              <span>简体中文</span>
            </div>
          </>
        )}
        {section === "runtime" && (
          <>
            <h2>Codex 更新策略</h2>
            <div className="setting-control">
              <div>
                <b>更新来源</b>
                <p>自动选择官方通道，或使用稳定镜像</p>
              </div>
              <div className="segmented">
                <button
                  className={
                    settings?.codexUpdateSource !== "mirror" ? "active" : ""
                  }
                  onClick={() => void save({ codexUpdateSource: "auto" })}
                >
                  自动
                </button>
                <button
                  className={
                    settings?.codexUpdateSource === "mirror" ? "active" : ""
                  }
                  onClick={() => void save({ codexUpdateSource: "mirror" })}
                >
                  镜像
                </button>
              </div>
            </div>
            <div className="setting-control">
              <div>
                <b>安装方式</b>
                <p>标准安装由 Windows 管理；免安装版由 Chimera++ 管理</p>
              </div>
              <div className="segmented">
                <button
                  className={
                    settings?.codexInstallMode !== "portable" ? "active" : ""
                  }
                  onClick={() => void save({ codexInstallMode: "standard" })}
                >
                  稳定版
                </button>
                <button
                  className={
                    settings?.codexInstallMode === "portable" ? "active" : ""
                  }
                  onClick={() => void save({ codexInstallMode: "portable" })}
                >
                  免安装版
                </button>
              </div>
            </div>
            <button
              className="setting-row"
              onClick={() =>
                void save({
                  checkCodexUpdatesOnStart: !settings?.checkCodexUpdatesOnStart,
                })
              }
            >
              <div>
                <b>启动时检查 Codex 更新</b>
                <p>仅检查，不会静默安装</p>
              </div>
              <span>
                {settings?.checkCodexUpdatesOnStart ? "开启" : "关闭"}
                <ChevronDown size={14} />
              </span>
            </button>
            {settings?.codexInstallMode === "portable" && (
              <button
                className="setting-row"
                onClick={() => void pickPortable()}
              >
                <div>
                  <b>免安装版目录</b>
                  <p title={settings.codexPortableRoot}>
                    {settings.codexPortableRoot || "使用 Chimera++ 默认目录"}
                  </p>
                </div>
                <FolderOpen size={16} />
              </button>
            )}
            <div className="settings-actions">
              <button onClick={onCheck}>
                <RefreshCw size={15} /> 立即检查 Codex 更新
              </button>
            </div>
          </>
        )}
        {section === "data" && (
          <>
            <h2>数据与隐私</h2>
            <button className="setting-row" onClick={() => void pickData()}>
              <div>
                <b>Chimera++ 数据目录</b>
                <p title={configPath}>{configPath || "读取中"}</p>
              </div>
              <FolderOpen size={16} />
            </button>
            <button
              className="setting-row"
              onClick={() => void settingsApi.openAppConfigFolder()}
            >
              <div>
                <b>打开数据目录</b>
                <p>查看配置、日志和本机备份</p>
              </div>
              <span>
                打开 <ChevronDown size={14} />
              </span>
            </button>
          </>
        )}
      </article>
    </section>
  );
}

function ConfirmOperation({
  action,
  onCancel,
  onConfirm,
}: {
  action: string;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  useEscapeClose(onCancel);
  const label =
    action === "update"
      ? "下载并安装更新"
      : action === "repair"
        ? "重新安装并修复 Codex"
        : action === "rollback"
          ? "回滚上一版本"
          : "卸载 Codex";
  return (
    <div className="modal-backdrop">
      <section
        className="confirm-dialog"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="runtime-confirm-title"
      >
        <CircleAlert size={26} />
        <h2 id="runtime-confirm-title">确认{label}？</h2>
        <p>
          该操作会修改 Codex 运行时。供应商配置和 `~/.codex`
          用户数据不会被删除。
        </p>
        <footer>
          <button onClick={onCancel}>取消</button>
          <button className="primary" onClick={onConfirm}>
            确认继续
          </button>
        </footer>
      </section>
    </div>
  );
}

function ConfirmProviderDelete({
  provider,
  onCancel,
  onConfirm,
}: {
  provider: Provider;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  useEscapeClose(onCancel);
  return (
    <div className="modal-backdrop">
      <section
        className="confirm-dialog"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="provider-delete-title"
      >
        <CircleAlert size={26} />
        <h2 id="provider-delete-title">确认删除“{provider.name}”？</h2>
        <p>该供应商会从 Chimera++ 中移除。当前 Codex 用户数据不会被删除。</p>
        <footer>
          <button onClick={onCancel}>取消</button>
          <button className="danger" onClick={onConfirm}>
            删除供应商
          </button>
        </footer>
      </section>
    </div>
  );
}

function DiagnosticsDialog({
  diagnostics,
  onClose,
}: {
  diagnostics: Diagnostic[];
  onClose: () => void;
}) {
  useEscapeClose(onClose);
  return (
    <div
      className="modal-backdrop"
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <section
        className="diagnostics-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="diagnostics-title"
      >
        <header>
          <div>
            <h2 id="diagnostics-title">Codex 诊断结果</h2>
            <p>结果由 Codex App Manager 运行时引擎生成。</p>
          </div>
          <button
            className="icon-button"
            aria-label="关闭诊断结果"
            onClick={onClose}
          >
            <X size={18} />
          </button>
        </header>
        <div className="diagnostics-list">
          {diagnostics.map((item) => (
            <article key={item.name}>
              <b>{item.name}</b>
              <p>{item.result}</p>
            </article>
          ))}
        </div>
        <footer>
          <button className="primary" onClick={onClose}>
            完成
          </button>
        </footer>
      </section>
    </div>
  );
}
function Onboarding({ onAdd }: { onAdd: () => void }) {
  return (
    <section className="onboarding">
      <div className="onboarding-brand">
        <Command size={18} /> Chimera++
      </div>
      <h2>开始配置你的 Codex</h2>
      <p>
        粘贴供应商地址和 API Key，Chimera++ 会获取模型列表并写入 Codex 配置。
      </p>
      <ol>
        <li>
          <b>1</b>
          <div>
            <strong>连接供应商</strong>
            <span>添加可编辑的 API 请求地址和密钥</span>
          </div>
        </li>
        <li>
          <b>2</b>
          <div>
            <strong>获取模型</strong>
            <span>从供应商接口读取模型，也可手动填写</span>
          </div>
        </li>
        <li>
          <b>3</b>
          <div>
            <strong>保存并应用</strong>
            <span>立即切换到新的 Codex 供应商</span>
          </div>
        </li>
      </ol>
      <button className="primary" onClick={onAdd}>
        开始配置
      </button>
    </section>
  );
}
function Field({
  label,
  value,
  onChange,
  placeholder,
  hint,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  hint?: string;
}) {
  return (
    <label>
      {label}
      <input
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
      />
      {hint && <small>{hint}</small>}
    </label>
  );
}
function Empty({
  label,
  action,
  onAction,
}: {
  label: string;
  action?: string;
  onAction?: () => void;
}) {
  return (
    <div className="empty">
      <p>{label}</p>
      {action && (
        <button className="primary" onClick={onAction}>
          {action}
        </button>
      )}
    </div>
  );
}
function Metric({
  label,
  value,
  detail,
  success = false,
}: {
  label: string;
  value: string;
  detail: string;
  success?: boolean;
}) {
  return (
    <article>
      <span>{label}</span>
      <b className={success ? "ok" : ""}>{value}</b>
      <small>{detail}</small>
    </article>
  );
}
function WindowControls() {
  const run = (action: "close" | "minimize" | "maximize") => {
    const window = getCurrentWindow();
    if (action === "close") void window.close();
    else if (action === "minimize") void window.minimize();
    else void window.toggleMaximize();
  };
  return (
    <div className="window-dots">
      <button aria-label="关闭 Chimera++" onClick={() => run("close")} />
      <button aria-label="最小化 Chimera++" onClick={() => run("minimize")} />
      <button
        aria-label="最大化或还原 Chimera++"
        onClick={() => run("maximize")}
      />
    </div>
  );
}
