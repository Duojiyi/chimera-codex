import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Activity,
  ArrowRight,
  BarChart3,
  Check,
  ChevronDown,
  CircleCheck,
  CircleAlert,
  Command,
  Download,
  Eye,
  EyeOff,
  LoaderCircle,
  FolderOpen,
  MoreHorizontal,
  Package,
  Paintbrush,
  Plus,
  Power,
  RefreshCw,
  Route,
  Search,
  Settings2,
  ShieldCheck,
  Trash2,
  Wrench,
  X,
  Zap,
} from "lucide-react";
import { toast } from "sonner";
import type {
  ClaudeApiKeyField,
  CodexApiFormat,
  CodexCatalogModel,
  Provider,
} from "@/types";
import { providersApi } from "@/lib/api/providers";
import { settingsApi } from "@/lib/api/settings";
import { vscodeApi } from "@/lib/api/vscode";
import { usageApi } from "@/lib/api/usage";
import { useUpdate } from "@/contexts/UpdateContext";
import type {
  DailyStats,
  ModelStats,
  RequestLog,
  UsageSummary,
} from "@/types/usage";
import type { Settings } from "@/types";
import { fetchModelsForConfig, type FetchedModel } from "@/lib/api/model-fetch";
import { getChimeraHubTemplate } from "@/config/codexTemplates";
import {
  extractCodexBaseUrl,
  extractCodexModelName,
  setCodexBaseUrl,
  setCodexModelName,
} from "@/utils/providerConfigUtils";
import { generateUUID } from "@/utils/uuid";
import {
  activityStorageKey,
  formatDuration,
  formatVersion,
  loadOperationRecords,
  resolveCurrentProvider,
  saveOperationRecords,
  setCodexProviderApiKey,
  type ConnectionState,
  type OperationRecord,
} from "./chimeraUtils";
import routeGateIcon from "@/assets/icons/chimera-route-gate.svg";
import "./chimera.css";

const runningInTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

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

type View = "providers" | "runtime" | "usage" | "appearance" | "settings";
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
  ["providers", "路由门", Route],
  ["runtime", "运行时", Package],
  ["usage", "词元", BarChart3],
  ["appearance", "外观", Paintbrush],
  ["settings", "设置", Settings2],
];

const runtimeText = (mode?: string | null) =>
  mode === "standard" ? "稳定版" : "免安装版";

function providerDraft(provider?: Provider | null) {
  const template = getChimeraHubTemplate();
  const config = String(provider?.settingsConfig?.config ?? template.config);
  const auth = (provider?.settingsConfig?.auth ?? template.auth) as Record<
    string,
    unknown
  >;
  const meta = provider?.meta ?? {};
  const apiFormat: CodexApiFormat =
    meta.apiFormat === "openai_chat" || meta.apiFormat === "anthropic"
      ? meta.apiFormat
      : "openai_responses";
  const anthropicAuthField: ClaudeApiKeyField =
    meta.apiKeyField === "ANTHROPIC_API_KEY"
      ? "ANTHROPIC_API_KEY"
      : "ANTHROPIC_AUTH_TOKEN";
  const catalogModels = Array.isArray(
    provider?.settingsConfig?.modelCatalog?.models,
  )
    ? provider.settingsConfig.modelCatalog.models
    : [];
  return {
    id: provider?.id ?? generateUUID(),
    name: provider?.name ?? template.name,
    websiteUrl: provider?.websiteUrl ?? template.websiteUrl,
    notes: provider?.notes ?? "",
    baseUrl: extractCodexBaseUrl(config) ?? "",
    apiKey: String(
      auth.OPENAI_API_KEY ?? auth[anthropicAuthField] ?? auth.api_key ?? "",
    ),
    model: extractCodexModelName(config) ?? "",
    config,
    auth,
    apiFormat,
    anthropicAuthField,
    impersonateClaudeCode: meta.impersonateClaudeCode === true,
    maxOutputTokens:
      typeof meta.maxOutputTokens === "number" && meta.maxOutputTokens > 0
        ? String(meta.maxOutputTokens)
        : "",
    isFullUrl: meta.isFullUrl === true,
    modelsUrl: typeof meta.modelsUrl === "string" ? meta.modelsUrl : "",
    customUserAgent:
      typeof meta.customUserAgent === "string" ? meta.customUserAgent : "",
    promptCacheRouting: meta.promptCacheRouting ?? "auto",
    codexChatReasoning: meta.codexChatReasoning ?? {},
    catalogModels: catalogModels as CodexCatalogModel[],
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
  const [modelFetchError, setModelFetchError] = useState<string | null>(null);
  const [fetchingModels, setFetchingModels] = useState(false);
  const [showKey, setShowKey] = useState(false);
  const [pendingAction, setPendingAction] = useState<
    "update" | "repair" | "rollback" | "uninstall" | null
  >(null);
  const [skinEnabled, setSkinEnabled] = useState(false);
  const [activity, setActivity] = useState<OperationRecord[]>([]);
  const activityKeyRef = useRef<string | null>(null);
  const startupProviderCheckRef = useRef(false);
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
  const [pendingSkinAction, setPendingSkinAction] = useState<{
    label: string;
    execute: () => void;
  } | null>(null);
  const [modelPickerOpen, setModelPickerOpen] = useState(false);
  const [onboardingDeferred, setOnboardingDeferred] = useState(false);
  void activity;

  const loadProviders = async () => {
    if (!runningInTauri) {
      const template = getChimeraHubTemplate();
      const previewProvider: Provider = {
        id: "preview-chimerahub",
        name: template.name,
        websiteUrl: template.websiteUrl,
        category: "custom",
        settingsConfig: { auth: template.auth, config: template.config },
      };
      setProviders([previewProvider]);
      setCurrentId(previewProvider.id);
      setCurrentSource("live");
      setLoading(false);
      return;
    }
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
    if (!runningInTauri) {
      setRuntime({
        installed: true,
        version: "26.721.41059",
        installMode: "standard",
        installPath: "预览模式 · 未访问本机文件",
        canRepair: true,
        canRollback: true,
        canUninstall: true,
      });
      return;
    }
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
    if (!runningInTauri) {
      setSkinEnabled(true);
      void loadProviders();
      void loadRuntime();
      return;
    }
    let active = true;
    void settingsApi
      .getAppConfigPath()
      .then((path) => {
        if (!active) return;
        const key = activityStorageKey(path);
        activityKeyRef.current = key;
        setActivity(loadOperationRecords(window.localStorage, key));
      })
      .catch(() => {
        // Activity history is optional; never fall back to a global profile.
        activityKeyRef.current = null;
      });
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
      active = false;
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
    setActivity((items) => {
      const records = [
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
      ];
      const key = activityKeyRef.current;
      return key
        ? saveOperationRecords(records, window.localStorage, key)
        : records;
    });
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

  useEffect(() => {
    if (!runningInTauri || loading || startupProviderCheckRef.current) return;
    const current = providers.find((provider) => provider.id === currentId);
    if (!current) return;
    startupProviderCheckRef.current = true;
    void settingsApi.get().then((settings) => {
      if (settings.checkProviderStatusOnStart === false) return;
      const endpoint = extractCodexBaseUrl(String(current.settingsConfig?.config ?? ""));
      if (endpoint) void testConnection(endpoint, current.name);
    }).catch(() => {
      // Startup validation is optional and must never block the main window.
    });
  }, [currentId, loading, providers]);

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
    const auth = setCodexProviderApiKey(editor.auth, editor.apiKey);
    const catalogModels = editor.catalogModels.filter((item) =>
      item.model.trim(),
    );
    const provider: Provider = {
      id: editor.id,
      name: editor.name.trim(),
      websiteUrl: editor.websiteUrl.trim() || undefined,
      notes: editor.notes.trim() || undefined,
      category: "custom",
      meta: {
        ...editor.original?.meta,
        apiFormat: editor.apiFormat,
        apiKeyField:
          editor.apiFormat === "anthropic"
            ? editor.anthropicAuthField
            : undefined,
        impersonateClaudeCode:
          editor.apiFormat === "anthropic" && editor.impersonateClaudeCode
            ? true
            : undefined,
        maxOutputTokens:
          editor.apiFormat === "anthropic" && Number(editor.maxOutputTokens) > 0
            ? Number(editor.maxOutputTokens)
            : undefined,
        isFullUrl: editor.isFullUrl || undefined,
        modelsUrl: editor.modelsUrl.trim() || undefined,
        customUserAgent: editor.customUserAgent.trim() || undefined,
        promptCacheRouting:
          editor.apiFormat === "openai_chat" &&
          editor.promptCacheRouting !== "auto"
            ? editor.promptCacheRouting
            : undefined,
        codexChatReasoning:
          editor.apiFormat === "openai_chat" &&
          (editor.codexChatReasoning.supportsThinking ||
            editor.codexChatReasoning.supportsEffort)
            ? editor.codexChatReasoning
            : undefined,
      },
      settingsConfig: {
        ...editor.original?.settingsConfig,
        auth,
        config,
        ...(catalogModels.length
          ? { modelCatalog: { models: catalogModels } }
          : models
            ? {
                modelCatalog: {
                  models: models.map((model) => ({
                    id: model.id,
                    name: model.id,
                  })),
                },
              }
            : { modelCatalog: undefined }),
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
      const result = await fetchModelsForConfig(
        editor.baseUrl,
        editor.apiKey,
        editor.isFullUrl,
        editor.modelsUrl.trim() || undefined,
        editor.customUserAgent.trim() || undefined,
      );
      setModels(result);
      setModelFetchError(
        result.length
          ? null
          : "供应商没有返回可选模型，可保留手动填写的模型名称。",
      );
      setModelPickerOpen(result.length > 0);
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
      setModelFetchError(
        "未能获取模型列表，请确认地址、密钥与供应商权限后重试。",
      );
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

  if (!loading && !providers.length && !editor && !onboardingDeferred) {
    return (
      <StandaloneOnboarding
        onAdd={() => setEditor(providerDraft())}
        onSkip={() => setOnboardingDeferred(true)}
      />
    );
  }

  return (
    <div className="chimera-shell">
      <main className="chimera-main">
        <header className="chimera-titlebar" data-tauri-drag-region>
          <div className="route-brand" data-tauri-drag-region>
            <span className="route-brand-mark"><img src={routeGateIcon} alt="" /></span>
            <strong>Chimera++</strong>
          </div>
          <div className="route-page-label"><span className="status-dot" />路由门</div>
          <div className="route-window-tools">
            <button aria-label="打开设置" onClick={() => setView("settings")}><Settings2 size={16} /></button>
            <WindowControls />
          </div>
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
        <h1 className="sr-only">{editor ? (editor.original ? "编辑供应商" : "添加供应商") : view === "providers" ? "路由门" : view === "runtime" ? "运行时" : view === "usage" ? "词元" : view === "appearance" ? "外观" : "设置"}</h1>
        <section className="chimera-content">
            {view === "providers" && (
              <NewProvidersView
                providers={providers}
                currentId={currentId}
                currentSource={currentSource}
                connection={connection}
                loading={loading}
                onSwitch={switchProvider}
                onEdit={(provider) => {
                  setModels(null);
                  setModelFetchError(null);
                  setEditor(providerDraft(provider));
                }}
                onAdd={() => {
                  setModels(null);
                  setModelFetchError(null);
                  setEditor(providerDraft());
                }}
              />
            )}
            {view === "runtime" && (
              <NewRuntimeView
                runtime={runtime}
                release={release}
                progress={downloadProgress}
                onCheck={checkRuntime}
                onDiagnose={diagnose}
                onAction={setPendingAction}
              />
            )}
            {view === "usage" && (
              <UsageView requests={requestLogs} />
            )}
            {view === "appearance" && (
              <AppearanceView
                enabled={skinEnabled}
                onRequestSkinAction={setPendingSkinAction}
              />
            )}
            {view === "settings" && <NewSettingsView />}
        </section>
        <nav className="route-bottom-nav" aria-label="主导航">
          {nav.map(([id, label, Icon]) => (
            <button key={id} className={view === id ? "is-active" : ""} onClick={() => setView(id)}>
              <span><Icon size={16} /></span><small>{label}</small>
            </button>
          ))}
        </nav>
        {editor && (
          <div className="provider-sheet-backdrop" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && setEditor(null)}>
            <ProviderEditor
              editor={editor}
              setEditor={setEditor}
              showKey={showKey}
              setShowKey={setShowKey}
              fetchingModels={fetchingModels}
              modelFetchError={modelFetchError}
              onFetchModels={fetchModels}
              onTest={() => void testConnection(editor.baseUrl, editor.name || "Codex")}
              onSave={saveProvider}
              onDelete={() => { if (editor.original) setPendingProviderDelete(editor.original); }}
              escapeDisabled={Boolean(pendingProviderDelete)}
            />
          </div>
        )}
      </main>
      {editor && models && modelPickerOpen && (
        <ModelPickerDialog
          models={models}
          selected={editor.model}
          onPick={(model) => {
            setEditor({ ...editor, model });
            setModelPickerOpen(false);
          }}
          onClose={() => setModelPickerOpen(false)}
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
      {pendingSkinAction && (
        <ConfirmSkinOperation
          label={pendingSkinAction.label}
          onCancel={() => setPendingSkinAction(null)}
          onConfirm={() => {
            const action = pendingSkinAction;
            setPendingSkinAction(null);
            action.execute();
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
                    ? "已识别当前 Codex 安装"
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

function NewRuntimeView({
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
  const [maintenanceOpen, setMaintenanceOpen] = useState(false);
  const [installMode, setInstallMode] = useState<"standard" | "portable">("standard");
  const [updateSource, setUpdateSource] = useState<"auto" | "mirror">("auto");
  const version = runtime?.version ?? "等待识别";
  const percent = progress?.total ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100)) : 0;
  useEffect(() => {
    setInstallMode(runtime?.installMode === "portable" ? "portable" : "standard");
    setUpdateSource(release?.source === "mirror" ? "mirror" : "auto");
  }, [runtime?.installMode, release?.source]);
  const saveRuntimePreference = async (patch: Partial<Settings>) => {
    if (!runningInTauri) return;
    try {
      const current = await settingsApi.get();
      await settingsApi.save({ ...current, ...patch });
      toast.success("运行时偏好已保存");
    } catch (reason) {
      toast.error("保存运行时偏好失败", { description: String(reason) });
    }
  };
  const openInstallDirectory = async () => {
    if (!runningInTauri) return;
    try { await invoke("open_codex_runtime_directory"); } catch (reason) { toast.error("无法打开安装目录", { description: String(reason) }); }
  };
  return <>
    <section className="runtime-reference-view">
      <span className="eyebrow">CODEX 运行时</span>
      <h1>本机 Codex 已准备就绪</h1>
      <div className="runtime-ring"><div><CircleCheck size={28} /><code>{version}</code><small>{runtime?.installed ? `${runtimeText(runtime.installMode)} · 稳定版` : "未检测到安装"}</small></div></div>
      <div className="runtime-info-strip"><div><FolderOpen size={16} /><span>安装位置<b>{runtime?.installed ? "已识别" : "未检测到"}</b></span></div><div><Download size={16} /><span>更新通道<b>{release?.source === "mirror" ? "镜像安装" : "稳定版"}</b></span></div><div><Activity size={16} /><span>自动检查<b>已开启</b></span></div></div>
      <div className="runtime-reference-actions"><button className="secondary" onClick={onCheck} disabled={Boolean(progress)}><RefreshCw size={14} />检查更新</button><button className="secondary" onClick={() => void openInstallDirectory()} disabled={!runtime?.installed}><FolderOpen size={14} />打开安装目录</button><button className="secondary" onClick={() => setMaintenanceOpen(true)}><Settings2 size={14} />管理更新源</button></div>
      {progress && <div className="runtime-reference-progress"><span>正在下载 {percent}%</span><i><u style={{ width: `${percent}%` }} /></i></div>}
    </section>
    {maintenanceOpen && <div className="provider-sheet-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) setMaintenanceOpen(false); }}><section className="runtime-maintenance-drawer" aria-label="安装与维护"><header><div><h2>安装与维护</h2><p>选择适合当前电脑的更新方式</p></div><button aria-label="关闭安装与维护" onClick={() => setMaintenanceOpen(false)}><X size={18} /></button></header><div className="runtime-maintenance-content"><b>安装方式</b><button className={`runtime-mode-card ${installMode === "standard" ? "is-active" : ""}`} onClick={() => { setInstallMode("standard"); void saveRuntimePreference({ codexInstallMode: "standard" }); }}><span><Download size={18} /></span><span><strong>标准安装</strong><small>自动集成到 Windows，适合大多数用户</small></span>{installMode === "standard" && <Check size={16} />}</button><button className={`runtime-mode-card ${installMode === "portable" ? "is-active" : ""}`} onClick={() => { setInstallMode("portable"); void saveRuntimePreference({ codexInstallMode: "portable" }); }}><span><Package size={18} /></span><span><strong>免安装版</strong><small>便携运行，可放在任意目录</small></span>{installMode === "portable" && <Check size={16} />}</button><b>更新源</b><div className="runtime-source-segment"><button className={updateSource === "auto" ? "is-active" : ""} onClick={() => { setUpdateSource("auto"); void saveRuntimePreference({ codexUpdateSource: "auto" }); }}>自动选择</button><button className={updateSource === "mirror" ? "is-active" : ""} onClick={() => { setUpdateSource("mirror"); void saveRuntimePreference({ codexUpdateSource: "mirror" }); }}>镜像安装</button></div><b>维护</b><div className="runtime-maintenance-list"><button onClick={onDiagnose}><Activity size={16} /><span><strong>诊断</strong><small>只检查，不修改本机文件</small></span><ChevronDown size={15} /></button><button onClick={() => onAction("repair")} disabled={!runtime?.canRepair}><Wrench size={16} /><span><strong>修复</strong><small>修复损坏的运行时组件</small></span><ChevronDown size={15} /></button><button onClick={() => onAction("rollback")} disabled={!runtime?.canRollback}><RefreshCw size={16} /><span><strong>回滚</strong><small>恢复上一个可用版本</small></span><ChevronDown size={15} /></button><button className="danger" onClick={() => onAction("uninstall")} disabled={!runtime?.canUninstall}><Trash2 size={16} /><span><strong>卸载 Codex</strong><small>保留 Chimera++ 与供应商配置</small></span><ChevronDown size={15} /></button></div></div><button className="primary runtime-maintenance-primary" onClick={() => onAction("update")} disabled={Boolean(progress)}><Download size={15} />下载并安装稳定版</button></section></div>}
  </>;
}

function NewProvidersView({
  providers,
  currentId,
  currentSource,
  connection,
  loading,
  onSwitch,
  onEdit,
  onAdd,
}: {
  providers: Provider[];
  currentId: string;
  currentSource: "live" | "stored" | "external" | "none";
  connection: ConnectionState;
  loading: boolean;
  onSwitch: (id: string) => void;
  onEdit: (provider: Provider) => void;
  onAdd: () => void;
}) {
  const [pickerOpen, setPickerOpen] = useState(false);
  const [query, setQuery] = useState("");
  if (loading) return <Empty label="正在读取供应商…" />;
  if (!providers.length) return <Onboarding onAdd={onAdd} />;
  const current = providers.find((provider) => provider.id === currentId) ?? providers[0];
  const endpoint = extractCodexBaseUrl(String(current.settingsConfig?.config ?? "")) || "未配置请求地址";
  const model = extractCodexModelName(String(current.settingsConfig?.config ?? "")) || "未设置";
  const connectionLabel = connection.kind === "connected" ? `已连接 · ${connection.message}` : connection.kind === "checking" ? "测试中" : connection.kind === "error" ? "连接失败" : currentSource === "live" ? "配置已识别" : "等待测试";
  const visibleProviders = providers.filter((provider) => {
    const haystack = `${provider.name} ${extractCodexModelName(String(provider.settingsConfig?.config ?? ""))}`.toLowerCase();
    return haystack.includes(query.trim().toLowerCase());
  });
  return (
    <section className="route-gate-view route-gate-reference">
      <div className="route-map" aria-label="当前 Codex 路由状态">
        <code className="route-stage-label">CODEX ROUTING</code>
        <div className="route-radar-field" aria-hidden="true" />
        <span className="route-stage-plus route-stage-plus-left">＋</span><span className="route-stage-plus route-stage-plus-right">＋</span>
        <div className="route-radar" aria-hidden="true"><span><span><i /></span></span></div>
        {pickerOpen && <div className="route-picker" role="dialog" aria-label="选择供应商"><label><Search size={15} /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索供应商或模型" autoFocus /></label><div>{visibleProviders.map((provider) => { const active = provider.id === current.id; const providerModel = extractCodexModelName(String(provider.settingsConfig?.config ?? "")) || "未设置模型"; const providerEndpoint = extractCodexBaseUrl(String(provider.settingsConfig?.config ?? "")) || "未配置地址"; return <button key={provider.id} className={active ? "is-active" : ""} onClick={() => { if (!active) onSwitch(provider.id); setPickerOpen(false); }}><span className="selector-mark"><Route size={16} /></span><span><b>{provider.name}</b><code>{providerModel} · {providerEndpoint}</code></span>{active && <Check size={16} />}</button>; })}</div><button className="route-picker-add" onClick={() => { setPickerOpen(false); onAdd(); }}><Plus size={14} />连接新供应商</button></div>}
        <div className="route-selector"><button className="selector-mark" aria-label="编辑当前供应商" onClick={() => onEdit(current)}><Route size={18} /></button><div><b>{current.name}</b><code>{model} · {endpoint}</code></div><button aria-label="选择供应商" onClick={() => setPickerOpen((value) => !value)}><ChevronDown size={17} /></button><button className="primary compact" onClick={() => void onSwitch(current.id)}><span>应用</span><ArrowRight size={15} /></button></div>
      </div>
      <div className="route-meta"><span>当前模型：<code>{model}</code></span><span>连接状态：<b className={connection.kind === "error" ? "error" : "ok"}>{connectionLabel}</b></span></div>
    </section>
  );
}

function ProviderEditor({
  editor,
  setEditor,
  showKey,
  setShowKey,
  fetchingModels,
  modelFetchError,
  onFetchModels,
  onTest,
  onSave,
  onDelete,
  escapeDisabled,
}: {
  editor: ReturnType<typeof providerDraft>;
  setEditor: (value: ReturnType<typeof providerDraft> | null) => void;
  showKey: boolean;
  setShowKey: (value: boolean) => void;
  fetchingModels: boolean;
  modelFetchError: string | null;
  onFetchModels: () => void;
  onTest: () => void;
  onSave: () => void;
  onDelete: () => void;
  escapeDisabled: boolean;
}) {
  useEscapeClose(() => setEditor(null), !escapeDisabled);
  const patch = (key: string, value: string) =>
    setEditor({ ...editor, [key]: value });
  return (
    <section
      className="provider-editor"
      aria-labelledby="provider-editor-title"
    >
      <header>
        <span className="provider-editor-mark">
          {(editor.name || "C").slice(0, 1).toUpperCase()}
        </span>
        <div>
          <h2 id="provider-editor-title">
            {editor.name || (editor.original ? "供应商" : "新供应商")}
          </h2>
          <p>保存后会写入 Codex 的当前供应商配置。</p>
        </div>
      </header>
      <div className="editor-form">
        {!editor.original && (
          <div className="provider-template" role="status">
            <div>
              <b>ChimeraHub 默认模板</b>
              <small>已填入 Responses 地址和默认模型；只需粘贴 API Key。</small>
            </div>
            <button
              type="button"
              className="secondary compact"
              onClick={() => setEditor(providerDraft())}
            >
              恢复模板
            </button>
          </div>
        )}
        <Field
          label="供应商名称"
          name="provider-name"
          value={editor.name}
          onChange={(value) => patch("name", value)}
          placeholder="例如 Chimera Hub"
        />
        <Field
          label="官网链接"
          name="provider-website"
          value={editor.websiteUrl}
          onChange={(value) => patch("websiteUrl", value)}
          placeholder="https://example.com"
        />
        <Field
          label="API 请求地址"
          name="provider-base-url"
          value={editor.baseUrl}
          onChange={(value) => patch("baseUrl", value)}
          placeholder="https://api.example.com/v1"
          hint="预设和自定义供应商都可编辑 URL。"
        />
        <label>
          API Key
          <div className="password-field">
            <input
              name="provider-api-key"
              autoComplete="off"
              spellCheck={false}
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
              name="provider-model"
              autoComplete="off"
              spellCheck={false}
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
        <details className="advanced-options">
          <summary>高级选项</summary>
          <div className="advanced-options-body">
            <p className="advanced-intro">
              仅在供应商不是标准 Responses API 时调整。保存后会应用到 Codex
              的兼容路由与配置。
            </p>
            <label>
              上游格式
              <select
                name="provider-api-format"
                value={editor.apiFormat}
                onChange={(event) => patch("apiFormat", event.target.value)}
              >
                <option value="openai_responses">
                  Responses（原生，推荐）
                </option>
                <option value="openai_chat">
                  Chat Completions（需路由接管）
                </option>
                <option value="anthropic">
                  Anthropic Messages（需路由接管）
                </option>
              </select>
              <small>
                Responses 可直连；Chat 与 Anthropic Messages 由本地路由转换为
                Codex 所需格式。
              </small>
            </label>
            <div className="advanced-grid">
              <label className="toggle-field">
                <span>
                  <b>完整 API 地址</b>
                  <small>地址已含完整请求路径时开启，不再自动补全路径。</small>
                </span>
                <input
                  name="provider-full-url"
                  type="checkbox"
                  checked={editor.isFullUrl}
                  onChange={(event) =>
                    setEditor({ ...editor, isFullUrl: event.target.checked })
                  }
                />
              </label>
              <label>
                模型列表地址（可选）
                <input
                  name="provider-models-url"
                  type="url"
                  autoComplete="url"
                  spellCheck={false}
                  value={editor.modelsUrl}
                  onChange={(event) => patch("modelsUrl", event.target.value)}
                  placeholder="https://api.example.com/v1/models"
                />
                <small>供应商的模型接口不同于主接口时填写。</small>
              </label>
              <label>
                自定义 User-Agent（可选）
                <input
                  name="provider-user-agent"
                  autoComplete="off"
                  spellCheck={false}
                  value={editor.customUserAgent}
                  onChange={(event) =>
                    patch("customUserAgent", event.target.value)
                  }
                  placeholder="留空使用默认请求标识"
                />
              </label>
            </div>
            {editor.apiFormat === "anthropic" && (
              <div className="advanced-group">
                <label>
                  Anthropic 认证字段
                  <select
                    name="provider-anthropic-auth"
                    value={editor.anthropicAuthField}
                    onChange={(event) =>
                      patch("anthropicAuthField", event.target.value)
                    }
                  >
                    <option value="ANTHROPIC_AUTH_TOKEN">
                      Authorization: Bearer
                    </option>
                    <option value="ANTHROPIC_API_KEY">x-api-key</option>
                  </select>
                </label>
                <label>
                  最大输出 tokens（可选）
                  <input
                    name="provider-max-output-tokens"
                    type="number"
                    min="1"
                    inputMode="numeric"
                    value={editor.maxOutputTokens}
                    onChange={(event) =>
                      patch(
                        "maxOutputTokens",
                        event.target.value.replace(/[^\d]/g, ""),
                      )
                    }
                    placeholder="默认 8192"
                  />
                </label>
                <label className="toggle-field">
                  <span>
                    <b>模拟 Claude Code 客户端</b>
                    <small>
                      仅当供应商明确要求 Claude Code 请求特征时开启。
                    </small>
                  </span>
                  <input
                    name="provider-impersonate-claude-code"
                    type="checkbox"
                    checked={editor.impersonateClaudeCode}
                    onChange={(event) =>
                      setEditor({
                        ...editor,
                        impersonateClaudeCode: event.target.checked,
                      })
                    }
                  />
                </label>
              </div>
            )}
            {editor.apiFormat === "openai_chat" && (
              <div className="advanced-group">
                <label>
                  提示词缓存路由
                  <select
                    name="provider-prompt-cache-routing"
                    value={editor.promptCacheRouting}
                    onChange={(event) =>
                      patch("promptCacheRouting", event.target.value)
                    }
                  >
                    <option value="auto">自动（推荐）</option>
                    <option value="enabled">开启</option>
                    <option value="disabled">关闭</option>
                  </select>
                  <small>严格网关遇到未知缓存字段时可选择关闭。</small>
                </label>
                <label className="toggle-field">
                  <span>
                    <b>支持思考模式</b>
                    <small>将 Codex 思考开关转换为上游 Chat 参数。</small>
                  </span>
                  <input
                    name="provider-supports-thinking"
                    type="checkbox"
                    checked={
                      editor.codexChatReasoning.supportsThinking === true
                    }
                    onChange={(event) =>
                      setEditor({
                        ...editor,
                        codexChatReasoning: {
                          ...editor.codexChatReasoning,
                          supportsThinking: event.target.checked,
                          supportsEffort: event.target.checked
                            ? editor.codexChatReasoning.supportsEffort
                            : false,
                        },
                      })
                    }
                  />
                </label>
                <label className="toggle-field">
                  <span>
                    <b>支持思考等级</b>
                    <small>支持 low、high、max 等推理强度时开启。</small>
                  </span>
                  <input
                    name="provider-supports-effort"
                    type="checkbox"
                    checked={editor.codexChatReasoning.supportsEffort === true}
                    onChange={(event) =>
                      setEditor({
                        ...editor,
                        codexChatReasoning: {
                          ...editor.codexChatReasoning,
                          supportsThinking: event.target.checked
                            ? true
                            : editor.codexChatReasoning.supportsThinking,
                          supportsEffort: event.target.checked,
                          effortParam: event.target.checked
                            ? (editor.codexChatReasoning.effortParam ??
                              "reasoning_effort")
                            : "none",
                        },
                      })
                    }
                  />
                </label>
              </div>
            )}
            <div className="advanced-group model-mapping">
              <div className="advanced-section-heading">
                <div>
                  <b>模型映射</b>
                  <small>
                    菜单显示名与实际请求模型可不同；留空则直接使用默认模型。
                  </small>
                </div>
                <button
                  type="button"
                  className="secondary compact"
                  onClick={() =>
                    setEditor({
                      ...editor,
                      catalogModels: [
                        ...editor.catalogModels,
                        { model: "", displayName: "", contextWindow: "" },
                      ],
                    })
                  }
                >
                  添加模型
                </button>
              </div>
              {editor.catalogModels.map((item, index) => (
                <div className="mapping-row" key={`${item.model}-${index}`}>
                  <input
                    aria-label="模型显示名"
                    value={item.displayName ?? ""}
                    onChange={(event) => {
                      const catalogModels = [...editor.catalogModels];
                      catalogModels[index] = {
                        ...item,
                        displayName: event.target.value,
                      };
                      setEditor({ ...editor, catalogModels });
                    }}
                    placeholder="菜单显示名"
                  />
                  <input
                    aria-label="实际请求模型"
                    value={item.model}
                    onChange={(event) => {
                      const catalogModels = [...editor.catalogModels];
                      catalogModels[index] = {
                        ...item,
                        model: event.target.value,
                      };
                      setEditor({ ...editor, catalogModels });
                    }}
                    placeholder="实际请求模型"
                  />
                  <input
                    aria-label="上下文窗口"
                    type="number"
                    min="1"
                    inputMode="numeric"
                    value={item.contextWindow ?? ""}
                    onChange={(event) => {
                      const catalogModels = [...editor.catalogModels];
                      catalogModels[index] = {
                        ...item,
                        contextWindow: event.target.value.replace(/[^\d]/g, ""),
                      };
                      setEditor({ ...editor, catalogModels });
                    }}
                    placeholder="上下文"
                  />
                  <button
                    type="button"
                    className="icon-button"
                    aria-label="删除模型映射"
                    onClick={() =>
                      setEditor({
                        ...editor,
                        catalogModels: editor.catalogModels.filter(
                          (_, i) => i !== index,
                        ),
                      })
                    }
                  >
                    <Trash2 size={15} />
                  </button>
                </div>
              ))}
            </div>
          </div>
        </details>
        {modelFetchError && (
          <p className="editor-model-error" role="status">
            <CircleAlert size={15} /> {modelFetchError}
          </p>
        )}
      </div>
      <footer>
        <button className="secondary" onClick={onTest}>
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
  );
}

function ModelPickerDialog({
  models,
  selected,
  onPick,
  onClose,
}: {
  models: FetchedModel[];
  selected: string;
  onPick: (model: string) => void;
  onClose: () => void;
}) {
  useEscapeClose(onClose);
  return (
    <div
      className="modal-backdrop"
      onMouseDown={(event) => event.target === event.currentTarget && onClose()}
    >
      <section
        className="model-picker"
        role="dialog"
        aria-modal="true"
        aria-labelledby="model-picker-title"
      >
        <header>
          <div>
            <h2 id="model-picker-title">选择默认模型</h2>
            <p>列表来自当前供应商接口。</p>
          </div>
          <button
            className="icon-button"
            aria-label="关闭模型列表"
            onClick={onClose}
          >
            <X size={18} />
          </button>
        </header>
        <div className="model-picker-list">
          {models.map((model) => (
            <button
              key={model.id}
              className={selected === model.id ? "picked" : ""}
              onClick={() => onPick(model.id)}
            >
              <span>{model.id}</span>
              {selected === model.id && <Check size={16} />}
            </button>
          ))}
        </div>
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
        <p>操作前会二次确认，并会保留 `~/.codex` 用户数据。</p>
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
          detail="本机路由请求记录"
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

function UsageView({ requests }: { requests: RequestLog[] }) {
  const [summary, setSummary] = useState<UsageSummary | null>(null);
  const [trends, setTrends] = useState<DailyStats[]>([]);
  const [models, setModels] = useState<ModelStats[]>([]);
  const [range, setRange] = useState<"today" | "7d" | "30d">("30d");
  const [error, setError] = useState("");

  useEffect(() => {
    if (!runningInTauri) {
      setSummary({ totalRequests: 1842, totalCost: "0", totalInputTokens: 2491880, totalOutputTokens: 792740, totalCacheCreationTokens: 0, totalCacheReadTokens: 0, successRate: 0.992, realTotalTokens: 3284620, cacheHitRate: 0 });
      setTrends(Array.from({ length: 24 }, (_, index) => ({ date: `2026-07-${String(index + 1).padStart(2, "0")}`, requestCount: 60 + index * 4, totalCost: "0", totalTokens: 90000 + index * 7300, totalInputTokens: 68000, totalOutputTokens: 22000, totalCacheCreationTokens: 0, totalCacheReadTokens: 0 })));
      setModels([{ model: "gpt-5.6-sol", requestCount: 1200, totalTokens: 2412800, totalCost: "0", avgCostPerRequest: "0" }, { model: "claude-sonnet-4", requestCount: 420, totalTokens: 642120, totalCost: "0", avgCostPerRequest: "0" }, { model: "其他模型", requestCount: 222, totalTokens: 229700, totalCost: "0", avgCostPerRequest: "0" }]);
      return;
    }
    const end = Date.now();
    const days = range === "today" ? 1 : range === "7d" ? 7 : 30;
    const start = end - days * 24 * 60 * 60 * 1000;
    let active = true;
    void Promise.all([
      usageApi.getUsageSummary(start, end, "codex"),
      usageApi.getUsageTrends(start, end, "codex"),
      usageApi.getModelStats(start, end, "codex"),
    ]).then(([nextSummary, nextTrends, nextModels]) => {
      if (!active) return;
      setSummary(nextSummary);
      setTrends(nextTrends);
      setModels(nextModels.slice(0, 3));
      setError("");
    }).catch((reason) => {
      if (active) setError(String(reason));
    });
    return () => { active = false; };
  }, [range]);

  const total = summary?.realTotalTokens ?? 0;
  const input = summary?.totalInputTokens ?? 0;
  const output = summary?.totalOutputTokens ?? 0;
  const max = Math.max(...trends.map((item) => item.totalTokens), 1);
  const modelTotal = Math.max(...models.map((item) => item.totalTokens), 1);
  const fallback = requests.reduce((sum, item) => sum + item.inputTokens + item.outputTokens, 0);
  return (
    <section className="usage-surface">
      <div className="usage-heading">
        <div><span className="eyebrow">词元统计</span><h1>了解 Codex 的词元消耗</h1><p>数据来自本机使用记录，不会上传。</p></div>
        <div className="range-segment">{([["today", "今日"], ["7d", "7 天"], ["30d", "30 天"]] as const).map(([id, label]) => <button key={id} className={range === id ? "is-active" : ""} onClick={() => setRange(id)}>{label}</button>)}</div>
      </div>
      {error && <div className="inline-error"><CircleAlert size={15} /> 词元统计暂时不可用：{error}</div>}
      <div className="usage-grid">
        <article className="usage-summary-card"><span>本期总用量</span><strong>{(total || fallback).toLocaleString("zh-CN")}</strong><small>词元</small><hr /><dl><div><dt>输入词元</dt><dd>{input.toLocaleString("zh-CN")}</dd></div><div><dt>输出词元</dt><dd>{output.toLocaleString("zh-CN")}</dd></div><div><dt>请求数</dt><dd>{summary?.totalRequests ?? requests.length}</dd></div></dl></article>
        <article className="usage-trend-card"><header><b>每日词元趋势</b><small>{summary ? `成功率 ${Math.round(summary.successRate * 100)}%` : "正在同步"}</small></header><div className="usage-chart" aria-label="每日词元趋势">{trends.length ? trends.map((item) => <span key={item.date} title={`${item.date} ${item.totalTokens.toLocaleString("zh-CN")} 词元`} style={{ height: `${Math.max(4, (item.totalTokens / max) * 100)}%` }} />) : <div className="chart-empty">暂无趋势数据</div>}</div><footer><small>较早</small><small>最近</small></footer></article>
      </div>
      <article className="usage-models"><header><b>模型消耗</b><small>按词元总量排序</small></header>{models.length ? models.map((item) => <div className="usage-model-row" key={item.model}><div><code>{item.model}</code><span>{item.totalTokens.toLocaleString("zh-CN")} 词元</span></div><strong>{Math.round((item.totalTokens / modelTotal) * 100)}%</strong><i><u style={{ width: `${Math.max(3, (item.totalTokens / modelTotal) * 100)}%` }} /></i></div>) : <p className="muted-copy">暂无模型统计。</p>}</article>
    </section>
  );
}

function AppearanceView({
  enabled,
  onRequestSkinAction,
}: {
  enabled: boolean;
  onRequestSkinAction: (action: { label: string; execute: () => void }) => void;
}) {
  const [skins, setSkins] = useState<CatalogSkin[]>([]);
  const [selectedId, setSelectedId] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState("");
  const load = async () => {
    if (!runningInTauri) {
      const preview = routeGateIcon;
      setSkins([
        { id: "preview-nerv", name: "NERV EVA-02", description: "Asuka Terminal", version: "1.4.2", author: "Chimera Market", preview, installed: true, applied: true },
        { id: "preview-oled", name: "OLED Mono", description: "Pure Black", version: "2.1.0", preview, installed: false, applied: false },
        { id: "preview-sakura", name: "Sakura Glass", description: "Soft Pink", version: "1.8.3", preview, installed: false, applied: false },
      ]);
      setSelectedId("preview-nerv");
      return;
    }
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
    <section className="skin-market-reference">
      <header className="skin-market-heading"><div><span className="eyebrow">CHATGPT 外观</span><h1>皮肤市场</h1><p>浏览、预览并安装 ChatGPT 客户端皮肤。</p></div><div className="skin-filter-tabs"><button className="is-active">精选</button><button>已安装</button><button>深色</button><button>浅色</button><button className="skin-import" onClick={() => void importLocal()} disabled={Boolean(busy)}>导入本地</button></div></header>
      <div className="skin-layout">
        <aside className="skin-list">
          {skins.map((skin) => (
            <button
              key={skin.id}
              className={skin.id === selectedId ? "active" : ""}
              onClick={() => setSelectedId(skin.id)}
            >
              <span className="skin-card-preview"><img src={skin.preview.startsWith("/") ? skin.preview : `https://skins.agentsmirror.com/${skin.preview.replace(/^\/+/, "")}`} alt="" /></span>
              <span><b>{skin.name}</b><small>{skin.description || `皮肤包 · ${skin.version}`}</small><code>v{skin.version}</code>{skin.installed && <em>{skin.applied ? "已安装" : "已下载"}</em>}</span>
            </button>
          ))}
          {!skins.length && !error && <Empty label="正在读取皮肤目录…" />}
          {error && <Empty label={`皮肤目录读取失败：${error}`} action="重试" onAction={() => void load()} />}
        </aside>
        <article className="skin-detail">
        {selected ? (
          <>
            <div className="skin-preview skin-preview-image">
              {selected.preview === routeGateIcon ? <div className="skin-preview-fallback"><aside><b>CHATGPT</b><span>新对话</span><span>Codex</span><span>设置</span></aside><main><code>{selected.name} // CODEX ROUTE</code><div><b>ChimeraHub 已连接</b><small>gpt-5.6-sol · 420 ms</small></div><footer>给 ChatGPT 发送消息 <i>↑</i></footer></main></div> : <img src={selected.preview.startsWith("/") ? selected.preview : `https://skins.agentsmirror.com/${selected.preview.replace(/^\/+/, "")}`} alt={`${selected.name} 预览`} />}
            </div>
            <div className="skin-detail-footer"><div><h2>{selected.name} {selected.description}</h2><p>{selected.installed ? `已安装 · v${selected.version} · 适配当前 ChatGPT` : `v${selected.version} · 可下载安装`}</p></div><div className="skin-actions">
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
                  className="primary"
                  onClick={() =>
                    onRequestSkinAction({
                      label: selected.applied ? "重新应用皮肤" : "应用皮肤",
                      execute: () =>
                        void run("应用皮肤", "apply_skin_package", {
                          skinId: selected.id,
                          confirm: true,
                        }),
                    })
                  }
                  disabled={Boolean(busy)}
                >
                  {selected.applied ? "重新应用" : "应用"}
                </button>
              )}
              {selected.installed && !selected.applied && <button
                className="secondary"
                onClick={() =>
                  onRequestSkinAction({
                    label: "试穿皮肤",
                    execute: () =>
                      void run("试穿", "try_skin_package", {
                        skinId: selected.id,
                        confirm: true,
                      }),
                  })
                }
                disabled={Boolean(busy) || !selected.installed}
              >
                试穿
              </button>}
              <button
                className="secondary"
                onClick={() =>
                  onRequestSkinAction({
                    label: "恢复默认外观",
                    execute: () =>
                      void run("恢复默认", "restore_skin_package", {
                        confirm: true,
                      }),
                  })
                }
                disabled={Boolean(busy)}
              >
                恢复默认
              </button>
            </div></div>
            <p className="integrity"><ShieldCheck size={16} /> 皮肤包经过 SHA256 完整性校验。</p>
          </>
        ) : (
          <Empty label="选择一个皮肤查看预览。" />
        )}
        </article>
      </div>
    </section>
  );
}

function NewSettingsView() {
  const [settings, setSettings] = useState<Settings | null>(null);
  useEffect(() => {
    if (!runningInTauri) { setSettings({ codexUpdateSource: "auto", codexInstallMode: "standard" } as Settings); return; }
    void settingsApi.get().then(setSettings).catch((reason) => toast.error("无法读取设置", { description: String(reason) }));
  }, []);
  const save = async (patch: Partial<Settings>) => { if (!settings) return; const next = { ...settings, ...patch }; if (!runningInTauri) { setSettings(next); return; } try { await settingsApi.save(next); setSettings(next); toast.success("设置已保存"); } catch (reason) { toast.error("设置保存失败", { description: String(reason) }); } };
  const updateChecks = settings?.checkCodexUpdatesOnStart ?? true;
  const providerChecks = settings?.checkProviderStatusOnStart ?? true;
  const minimizeToTray = settings?.minimizeToTrayOnClose ?? false;
  const openDataFolder = async () => {
    if (!runningInTauri) return;
    try { await settingsApi.openAppConfigFolder(); } catch (reason) { toast.error("无法打开数据目录", { description: String(reason) }); }
  };
  return <section className="new-settings-view"><span className="eyebrow">设置</span><h1>保持简单，也保留控制权</h1><div className="settings-reference-list"><button className="settings-reference-row" onClick={() => void save({ checkCodexUpdatesOnStart: !updateChecks })}><span><b>启动时检查 Codex 更新</b><small>仅提醒，不会静默替换当前版本</small></span><i className={`settings-switch ${updateChecks ? "is-on" : ""}`}><u /></i></button><button className="settings-reference-row" onClick={() => void save({ checkProviderStatusOnStart: !providerChecks })}><span><b>自动检查供应商状态</b><small>启动后轻量验证当前路由</small></span><i className={`settings-switch ${providerChecks ? "is-on" : ""}`}><u /></i></button><button className="settings-reference-row" onClick={() => void save({ minimizeToTrayOnClose: !minimizeToTray })}><span><b>关闭窗口后最小化到托盘</b><small>保留快速切换能力</small></span><i className={`settings-switch ${minimizeToTray ? "is-on" : ""}`}><u /></i></button><div className="settings-reference-row settings-segment-row"><span><b>更新通道</b><small>只提供稳定版和免安装版</small></span><div className="settings-segment"><button className={settings?.codexUpdateSource !== "mirror" ? "is-active" : ""} onClick={() => void save({ codexUpdateSource: "auto" })}>稳定版</button><button className={settings?.codexUpdateSource === "mirror" ? "is-active" : ""} onClick={() => void save({ codexUpdateSource: "mirror" })}>免安装版</button></div></div><button className="settings-reference-row settings-link-row" onClick={() => void openDataFolder()}><span><b>数据与日志</b><small>配置保存在本机</small></span><ChevronDown size={16} /></button></div><footer className="settings-reference-footer"><code>Chimera++ 2.0.4</code><button className="secondary" onClick={() => void save({ codexUpdateSource: "auto", codexInstallMode: "standard", checkCodexUpdatesOnStart: true, checkProviderStatusOnStart: true, minimizeToTrayOnClose: false })}>恢复默认设置</button></footer></section>;
}

function SettingsView({ onCheck }: { onCheck: () => void }) {
  const [section, setSection] = useState<
    "general" | "runtime" | "data" | "advanced"
  >("general");
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
          ["data", "数据与隐私"],
          ["runtime", "更新策略"],
          ["advanced", "高级"],
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
        {section === "advanced" && (
          <>
            <h2>高级</h2>
            <div className="setting-row">
              <div>
                <b>Codex 免安装版目录</b>
                <p title={settings?.codexPortableRoot || undefined}>
                  {settings?.codexPortableRoot || "自动识别默认安装目录"}
                </p>
              </div>
              <span>只在免安装模式下使用</span>
            </div>
            <div className="setting-row">
              <div>
                <b>运行时操作保护</b>
                <p>升级、修复、回滚、卸载和皮肤应用均要求二次确认</p>
              </div>
              <span>已启用</span>
            </div>
            <div className="settings-actions">
              <button onClick={onCheck}>
                <RefreshCw size={15} /> 检查 Codex 更新
              </button>
            </div>
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

function ConfirmSkinOperation({
  label,
  onCancel,
  onConfirm,
}: {
  label: string;
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
        aria-labelledby="skin-confirm-title"
      >
        <Paintbrush size={26} />
        <h2 id="skin-confirm-title">确认{label}？</h2>
        <p>该操作会关闭并重新启动 Codex。供应商配置和用户数据不会被修改。</p>
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
            <p>检测结果来自当前系统的 Codex 安装状态。</p>
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
        <img src={routeGateIcon} alt="" /> Chimera++
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

function StandaloneOnboarding({
  onAdd,
  onSkip,
}: {
  onAdd: () => void;
  onSkip: () => void;
}) {
  return (
    <main className="onboarding-screen">
      <section className="onboarding onboarding-card">
        <div className="onboarding-brand">
          <img src={routeGateIcon} alt="" /> Chimera++
        </div>
        <h1>开始配置你的 Codex</h1>
        <p>
          只需填写一次供应商地址和密钥，之后可在控制台快速切换。Chimera++
          会自动识别本机 Codex 安装方式并同步模型列表。
        </p>
        <ol>
          <li>
            <b>1</b>
            <div>
              <strong>连接供应商</strong>
              <span>粘贴接口地址和 API 密钥</span>
            </div>
          </li>
          <li>
            <b>2</b>
            <div>
              <strong>检测 Codex</strong>
              <span>识别标准安装或免安装版本</span>
            </div>
          </li>
          <li>
            <b>3</b>
            <div>
              <strong>完成设置</strong>
              <span>保存后即可快速切换</span>
            </div>
          </li>
        </ol>
        <footer>
          <button className="secondary" onClick={onSkip}>
            稍后配置
          </button>
          <button className="primary" onClick={onAdd}>
            开始配置
          </button>
        </footer>
        <small>Chimera++ 2.0 · 数据仅保存在本机</small>
      </section>
    </main>
  );
}
function Field({
  label,
  name,
  value,
  onChange,
  placeholder,
  hint,
}: {
  label: string;
  name: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  hint?: string;
}) {
  return (
    <label>
      {label}
      <input
        name={name}
        autoComplete="off"
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
void ProvidersView;
void ActivityView;
void RuntimeView;
void SettingsView;

function WindowControls() {
  return (
    <div className="window-dots">
      <button aria-label="关闭 Chimera++" onClick={() => void getCurrentWindow().close()}>
        <Power size={16} />
      </button>
    </div>
  );
}
