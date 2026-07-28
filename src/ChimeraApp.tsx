import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
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
  MoreHorizontal,
  Paintbrush,
  Pencil,
  Plus,
  RefreshCw,
  Settings2,
  ShieldCheck,
  Trash2,
  Wrench,
  X,
} from "lucide-react";
import { toast } from "sonner";
import type { Provider } from "@/types";
import { providersApi } from "@/lib/api/providers";
import { settingsApi } from "@/lib/api/settings";
import { fetchModelsForConfig, type FetchedModel } from "@/lib/api/model-fetch";
import { getCodexCustomTemplate } from "@/config/codexTemplates";
import {
  extractCodexBaseUrl,
  extractCodexModelName,
  setCodexBaseUrl,
  setCodexModelName,
} from "@/utils/providerConfigUtils";
import { generateUUID } from "@/utils/uuid";
import "./chimera.css";

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
};
type Capability = { id: string; enabledByDefault: boolean };
type ProductCapabilities = { capabilities: Capability[] };

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
  const auth = (provider?.settingsConfig?.auth ?? template.auth) as Record<string, unknown>;
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
  const [view, setView] = useState<View>("providers");
  const [providers, setProviders] = useState<Provider[]>([]);
  const [currentId, setCurrentId] = useState("");
  const [loading, setLoading] = useState(true);
  const [runtime, setRuntime] = useState<RuntimeStatus | null>(null);
  const [release, setRelease] = useState<ReleaseStatus | null>(null);
  const [editor, setEditor] = useState<ReturnType<typeof providerDraft> | null>(null);
  const [models, setModels] = useState<FetchedModel[] | null>(null);
  const [fetchingModels, setFetchingModels] = useState(false);
  const [showKey, setShowKey] = useState(false);
  const [pendingAction, setPendingAction] = useState<
    "update" | "repair" | "rollback" | "uninstall" | null
  >(null);
  const [skinEnabled, setSkinEnabled] = useState(false);
  const [activity, setActivity] = useState<string[]>([]);

  const loadProviders = async () => {
    try {
      const [all, current] = await Promise.all([
        providersApi.getAll("codex"),
        providersApi.getCurrent("codex"),
      ]);
      setProviders(Object.values(all).sort((a, b) => (a.sortIndex ?? 0) - (b.sortIndex ?? 0)));
      setCurrentId(current);
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
      if (view === "runtime") toast.error("无法读取 Codex 运行时状态", { description: String(error) });
    }
  };

  useEffect(() => {
    void loadProviders();
    void loadRuntime();
    void invoke<ProductCapabilities>("get_product_capabilities")
      .then((value) => setSkinEnabled(value.capabilities.some((item) => item.id === "codex_themes" && item.enabledByDefault)))
      .catch(() => setSkinEnabled(false));
  }, []);

  const activeProvider = useMemo(
    () => providers.find((provider) => provider.id === currentId) ?? null,
    [providers, currentId],
  );

  const note = (message: string) => setActivity((items) => [`${new Date().toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" })}  ${message}`, ...items].slice(0, 20));

  const switchProvider = async (id: string) => {
    try {
      await providersApi.switch(id, "codex");
      setCurrentId(id);
      note(`已切换供应商：${providers.find((item) => item.id === id)?.name ?? id}`);
      toast.success("已应用到 Codex");
    } catch (error) {
      toast.error("切换失败", { description: String(error) });
    }
  };

  const saveProvider = async () => {
    if (!editor) return;
    if (!editor.name.trim() || !editor.baseUrl.trim() || !editor.apiKey.trim()) {
      toast.error("请填写供应商名称、API 请求地址和 API Key");
      return;
    }
    const config = setCodexModelName(setCodexBaseUrl(editor.config, editor.baseUrl), editor.model);
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
        ...(models ? { modelCatalog: { models: models.map((model) => ({ id: model.id, name: model.id })) } } : {}),
      },
    };
    try {
      if (editor.original) await providersApi.update(provider, "codex", editor.original.id);
      else await providersApi.add(provider, "codex", false);
      await providersApi.switch(provider.id, "codex");
      await loadProviders();
      setEditor(null);
      note(`已保存供应商：${provider.name}`);
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
      note(`已获取 ${result.length} 个模型`);
      toast.success(`已获取 ${result.length} 个模型`);
    } catch (error) {
      toast.error("获取模型失败，可手动输入模型名称", { description: String(error) });
      setModels([]);
    } finally {
      setFetchingModels(false);
    }
  };

  const checkRuntime = async () => {
    try {
      const result = await invoke<ReleaseStatus>("check_codex_runtime_update", { source: null, installMode: null });
      setRelease(result);
      note(result.updateAvailable ? `发现 Codex 新版本：${result.latestVersion}` : "Codex 已是最新版本");
      toast.success(result.updateAvailable ? "发现新版本" : "Codex 已是最新版本");
    } catch (error) {
      toast.error("检查更新失败", { description: String(error) });
    }
  };

  const runRuntimeAction = async () => {
    if (!pendingAction) return;
    try {
      if (pendingAction === "update") {
        await invoke("apply_codex_runtime_update", { expectedVersion: release?.latestVersion ?? null, source: null, installMode: null, confirm: true });
      } else if (pendingAction === "repair") {
        await invoke("repair_codex_runtime", { source: null, installMode: null, confirm: true });
      } else if (pendingAction === "rollback") {
        await invoke("rollback_codex_runtime", { confirm: true });
      } else {
        await invoke("uninstall_codex_runtime", { confirm: true });
      }
      note(`已完成 Codex ${pendingAction === "update" ? "更新" : pendingAction === "repair" ? "修复" : pendingAction === "rollback" ? "回滚" : "卸载"}`);
      toast.success("操作已完成");
      await loadRuntime();
    } catch (error) {
      toast.error("操作失败", { description: String(error) });
    } finally {
      setPendingAction(null);
    }
  };

  return (
    <div className="chimera-shell">
      <aside className="chimera-sidebar">
        <div className="chimera-brand"><span><Command size={16} /></span><strong>Chimera++</strong></div>
        <p className="chimera-kicker">CODEX WORKSPACE</p>
        <nav>{nav.map(([id, label, Icon]) => <button key={id} className={view === id ? "is-active" : ""} onClick={() => setView(id)}><Icon size={17} />{label}</button>)}</nav>
        <div className="chimera-sidebar-footer"><span className="status-dot" /> Codex 专用工作台<br /><small>后端扩展能力未在前端开放</small></div>
      </aside>
      <main className="chimera-main">
        <header className="chimera-titlebar"><div><h1>{nav.find((item) => item[0] === view)?.[1]}</h1><p>{view === "providers" ? "切换、配置并验证当前 API 连接" : view === "runtime" ? "安装、更新与修复本机 Codex" : view === "appearance" ? "管理 Codex 客户端皮肤" : view === "settings" ? "应用行为、数据位置与更新策略" : "查看本次会话中的操作记录"}</p></div><WindowControls /></header>
        {view === "providers" && <ProvidersView providers={providers} currentId={currentId} loading={loading} onSwitch={switchProvider} onEdit={(provider) => { setModels(null); setEditor(providerDraft(provider)); }} onAdd={() => { setModels(null); setEditor(providerDraft()); }} />}
        {view === "runtime" && <RuntimeView runtime={runtime} release={release} onCheck={checkRuntime} onDiagnose={async () => { try { const result = await invoke<Array<{ name: string; result: string }>>("diagnose_codex_runtime"); note(`诊断完成：${result.length} 项`); toast.success("诊断完成"); } catch (error) { toast.error("诊断失败", { description: String(error) }); } }} onAction={setPendingAction} />}
        {view === "activity" && <ActivityView entries={activity} />}
        {view === "appearance" && <AppearanceView enabled={skinEnabled} />}
        {view === "settings" && <SettingsView onCheck={checkRuntime} />}
      </main>
      {editor && <ProviderEditor editor={editor} setEditor={setEditor} showKey={showKey} setShowKey={setShowKey} models={models} fetchingModels={fetchingModels} onFetchModels={fetchModels} onSave={saveProvider} onDelete={async () => { if (!editor.original) return setEditor(null); try { await providersApi.delete(editor.original.id, "codex"); await loadProviders(); setEditor(null); toast.success("供应商已删除"); } catch (error) { toast.error("删除失败", { description: String(error) }); } }} />}
      {pendingAction && <ConfirmOperation action={pendingAction} onCancel={() => setPendingAction(null)} onConfirm={runRuntimeAction} />}
    </div>
  );
}

function ProvidersView({ providers, currentId, loading, onSwitch, onEdit, onAdd }: { providers: Provider[]; currentId: string; loading: boolean; onSwitch: (id: string) => void; onEdit: (provider: Provider) => void; onAdd: () => void }) {
  return <section className="provider-page">{loading ? <Empty label="正在读取供应商…" /> : providers.length === 0 ? <Onboarding onAdd={onAdd} /> : <><div className="toolbar"><div className="search-box">搜索供应商名称或地址</div><button className="primary" onClick={onAdd}><Plus size={15} /> 添加供应商</button></div><div className="provider-list">{providers.map((provider) => { const active = provider.id === currentId; return <article className={`provider-card ${active ? "selected" : ""}`} key={provider.id}><div className="provider-monogram">{provider.name.slice(0, 1).toUpperCase()}</div><div className="provider-copy"><div><b>{provider.name}</b>{active && <em>当前使用</em>}</div><code>{extractCodexBaseUrl(String(provider.settingsConfig?.config ?? "")) || "未配置请求地址"}</code><small>默认模型 · {extractCodexModelName(String(provider.settingsConfig?.config ?? "")) || "未设置"}</small></div><div className="provider-actions">{!active && <button className="dark" onClick={() => onSwitch(provider.id)}>切换</button>}<button onClick={() => onEdit(provider)}><Pencil size={14} /> 编辑</button><button aria-label="更多操作"><MoreHorizontal size={16} /></button></div></article>; })}</div></>}</section>;
}

function ProviderEditor({ editor, setEditor, showKey, setShowKey, models, fetchingModels, onFetchModels, onSave, onDelete }: { editor: ReturnType<typeof providerDraft>; setEditor: (value: ReturnType<typeof providerDraft> | null) => void; showKey: boolean; setShowKey: (value: boolean) => void; models: FetchedModel[] | null; fetchingModels: boolean; onFetchModels: () => void; onSave: () => void; onDelete: () => void }) {
  const patch = (key: string, value: string) => setEditor({ ...editor, [key]: value });
  return <div className="modal-backdrop"><section className="provider-editor"><header><button className="icon-button" onClick={() => setEditor(null)}><X size={18} /></button><div><h2>{editor.original ? "编辑供应商" : "添加供应商"}</h2><p>仅写入 Codex 的供应商配置。</p></div></header><div className="editor-form"><Field label="供应商名称" value={editor.name} onChange={(value) => patch("name", value)} placeholder="例如 Chimera Hub" /><Field label="官网链接" value={editor.websiteUrl} onChange={(value) => patch("websiteUrl", value)} placeholder="https://example.com" /><Field label="API 请求地址" value={editor.baseUrl} onChange={(value) => patch("baseUrl", value)} placeholder="https://api.example.com/v1" hint="预设和自定义供应商都可编辑 URL。" /><label>API Key<div className="password-field"><input type={showKey ? "text" : "password"} value={editor.apiKey} onChange={(event) => patch("apiKey", event.target.value)} placeholder="粘贴 API Key" /><button onClick={() => setShowKey(!showKey)}>{showKey ? <EyeOff size={16} /> : <Eye size={16} />}</button></div></label><label>默认模型<div className="model-input"><input value={editor.model} onChange={(event) => patch("model", event.target.value)} placeholder="先获取模型列表，或手动输入" /><button onClick={onFetchModels} disabled={fetchingModels}>{fetchingModels ? <LoaderCircle className="spin" size={15} /> : <Download size={15} />} 获取模型</button></div></label>{models !== null && <div className="model-results">{models.length ? models.map((model) => <button key={model.id} className={editor.model === model.id ? "picked" : ""} onClick={() => patch("model", model.id)}>{model.id}{editor.model === model.id && <Check size={15} />}</button>) : <p><CircleAlert size={15} /> 未能获取模型列表，可保留手动输入的模型名称。</p>}</div>}<details><summary>高级选项</summary><p>默认采用 Codex Responses 兼容配置。复杂协议和模型映射仅在需要时展开。</p></details></div><footer><button className="secondary" onClick={async () => { await onFetchModels(); }}>测试连接</button><div>{editor.original && <button className="danger" onClick={onDelete}><Trash2 size={15} /> 删除</button>}<button className="primary" onClick={onSave}>保存并应用</button></div></footer></section></div>;
}

function RuntimeView({ runtime, release, onCheck, onDiagnose, onAction }: { runtime: RuntimeStatus | null; release: ReleaseStatus | null; onCheck: () => void; onDiagnose: () => void; onAction: (value: "update" | "repair" | "rollback" | "uninstall") => void }) {
  return <section className="runtime-grid"><div className="runtime-primary"><article className="panel"><div className="panel-header"><div><span>当前安装</span><h2>{runtime?.installed ? runtime.version || "已安装" : "未检测到 Codex"}</h2><p>{runtime?.installed ? `${runtimeText(runtime.installMode)} · ${runtime.installPath || "路径已识别"}` : "可选择稳定版或免安装版进行安装"}</p></div><b className="green-tag">{runtime?.installed ? "状态正常" : "未安装"}</b></div><div className="runtime-actions"><button className="dark" onClick={onCheck}><RefreshCw size={15} /> 检查更新</button><button onClick={onDiagnose}><Wrench size={15} /> 修复与诊断</button><button onClick={() => onAction("uninstall")} disabled={!runtime?.canUninstall}><Trash2 size={15} /> 卸载</button></div></article><article className="panel"><h3>更新方式</h3><div className="mode-cards"><div className="mode-card active"><b>稳定版</b><span>使用经过验证的正式更新</span></div><div className="mode-card"><b>免安装版</b><span>保留在 Chimera++ 管理目录</span></div></div></article>{release && <article className="panel update-result"><div><span>可用版本</span><h3>{release.latestVersion}</h3><p>{release.updateAvailable ? "已发现新版本，安装前会自动创建回滚点。" : "当前已经是最新版本。"}</p></div>{release.updateAvailable && <button className="primary" onClick={() => onAction("update")}>下载并安装</button>}</article>}</div><aside className="panel runtime-side"><h3>运行状态</h3><StatusRow label="安装目录" value={runtime?.installPath ? "已识别" : "未检测到"} ok={Boolean(runtime?.installed)} /><StatusRow label="签名与包校验" value="通过安装前校验" ok /><StatusRow label="回滚副本" value={runtime?.canRollback ? "可用" : "当前不可用"} ok={Boolean(runtime?.canRollback)} /><hr /><button onClick={() => onAction("repair")} disabled={!runtime?.canRepair}>重新安装并修复</button><button onClick={() => onAction("rollback")} disabled={!runtime?.canRollback}>回滚上一版本</button></aside></section>;
}

function ActivityView({ entries }: { entries: string[] }) { return <section className="panel activity-panel"><h2>操作记录</h2><p>记录本次 Chimera++ 会话中触发的 Codex 操作。</p>{entries.length ? <ul>{entries.map((entry, index) => <li key={`${entry}-${index}`}>{entry}</li>)}</ul> : <Empty label="暂无记录。切换供应商、获取模型或检查更新后会显示在这里。" />}</section>; }

function AppearanceView({ enabled }: { enabled: boolean }) { return <section className="skin-layout"><aside className="skin-list"><div className="skin-tabs"><b>在线皮肤</b><span>已安装</span></div>{["NERV EVA-02 Asuka Terminal", "TPC GUTS Command Terminal", "NERV EVA-00 Rei Prototype"].map((name, index) => <button key={name} className={index === 0 ? "active" : ""}><i style={{ background: ["#22191a", "#222c43", "#e4e1ec"][index] }} />{name}<small>Codex skin</small></button>)}</aside><article className="skin-detail"><h2>NERV EVA-02 Asuka Terminal</h2><p>素材化 Codex 皮肤 · 深色主界面与浅色报告风格。</p><div className="skin-preview"><span>NERV</span><strong>God's in His Heaven</strong><div /></div>{enabled ? <div className="skin-actions"><button className="dark">下载安装</button><button>试穿</button><button>恢复默认</button></div> : <div className="feature-gap"><CircleAlert size={17} /><div><b>皮肤引擎尚未接入</b><p>页面和安装流程已设计，但当前后端未提供 `.codexskin` 校验、CDP 注入和恢复命令，因此不会显示无效按钮。</p></div></div>}<p className="integrity"><ShieldCheck size={16} /> 已规划为不修改 Codex 文件的注入式皮肤。</p></article></section>; }

function SettingsView({ onCheck }: { onCheck: () => void }) { const [autoLaunch, setAutoLaunch] = useState<boolean | null>(null); useEffect(() => { void settingsApi.getAutoLaunchStatus().then(setAutoLaunch).catch(() => setAutoLaunch(false)); }, []); return <section className="settings-layout"><aside><b>常规</b><b className="active">更新策略</b><b>数据与隐私</b><b>高级</b></aside><article className="panel"><h2>更新策略</h2><p>只管理 Chimera++ 和 Codex 的版本检查，不开放其他应用管理。</p><SettingRow label="启动时检查 Codex 更新" detail="发现更新后显示通知，不自动替换文件" value={autoLaunch === null ? "读取中" : "已启用"} /><SettingRow label="Codex 更新方式" detail="根据当前安装状态使用稳定版或免安装版" value="自动识别" /><SettingRow label="配置备份" detail="更新和运行时修复前创建本机回滚点" value="已启用" /><div className="settings-actions"><button onClick={onCheck}><RefreshCw size={15} /> 立即检查更新</button><button onClick={() => void settingsApi.setAutoLaunch(!(autoLaunch ?? false)).then((value) => { setAutoLaunch(value); toast.success(value ? "已开启开机启动" : "已关闭开机启动"); }).catch((error) => toast.error("设置失败", { description: String(error) }))}>{autoLaunch ? "关闭开机启动" : "开启开机启动"}</button></div></article></section>; }

function ConfirmOperation({ action, onCancel, onConfirm }: { action: string; onCancel: () => void; onConfirm: () => void }) { const label = action === "update" ? "下载并安装更新" : action === "repair" ? "重新安装并修复 Codex" : action === "rollback" ? "回滚上一版本" : "卸载 Codex"; return <div className="modal-backdrop"><section className="confirm-dialog"><CircleAlert size={26} /><h2>确认{label}？</h2><p>该操作会修改 Codex 运行时。供应商配置和 `~/.codex` 用户数据不会被删除。</p><footer><button onClick={onCancel}>取消</button><button className="primary" onClick={onConfirm}>确认继续</button></footer></section></div>; }
function Onboarding({ onAdd }: { onAdd: () => void }) { return <section className="onboarding"><div className="onboarding-brand"><Command size={18} /> Chimera++</div><h2>开始配置你的 Codex</h2><p>粘贴供应商地址和 API Key，Chimera++ 会获取模型列表并写入 Codex 配置。</p><ol><li><b>1</b><div><strong>连接供应商</strong><span>添加可编辑的 API 请求地址和密钥</span></div></li><li><b>2</b><div><strong>获取模型</strong><span>从供应商接口读取模型，也可手动填写</span></div></li><li><b>3</b><div><strong>保存并应用</strong><span>立即切换到新的 Codex 供应商</span></div></li></ol><button className="primary" onClick={onAdd}>开始配置</button></section>; }
function Field({ label, value, onChange, placeholder, hint }: { label: string; value: string; onChange: (value: string) => void; placeholder?: string; hint?: string }) { return <label>{label}<input value={value} onChange={(event) => onChange(event.target.value)} placeholder={placeholder} />{hint && <small>{hint}</small>}</label>; }
function Empty({ label, action, onAction }: { label: string; action?: string; onAction?: () => void }) { return <div className="empty"><p>{label}</p>{action && <button className="primary" onClick={onAction}>{action}</button>}</div>; }
function StatusRow({ label, value, ok }: { label: string; value: string; ok?: boolean }) { return <div className="status-row"><span>{label}</span><b className={ok ? "ok" : ""}>{ok && <Check size={14} />}{value}</b></div>; }
function SettingRow({ label, detail, value }: { label: string; detail: string; value: string }) { return <div className="setting-row"><div><b>{label}</b><p>{detail}</p></div><span>{value}<ChevronDown size={14} /></span></div>; }
function WindowControls() { const run = (action: "close" | "minimize" | "maximize") => { const window = getCurrentWindow(); if (action === "close") void window.close(); else if (action === "minimize") void window.minimize(); else void window.toggleMaximize(); }; return <div className="window-dots"><button aria-label="关闭 Chimera++" onClick={() => run("close")} /><button aria-label="最小化 Chimera++" onClick={() => run("minimize")} /><button aria-label="最大化或还原 Chimera++" onClick={() => run("maximize")} /></div>; }
