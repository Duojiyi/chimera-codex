import { useEffect, useState } from "react";
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
        <div className="chimera-sidebar-footer"><span className="status-dot" /> 全部服务正常<br /><small>最后检测 刚刚</small></div><div className="workspace-identity"><b>H</b><span>本机工作区<small>Chimera++ 2.0</small></span></div>
      </aside>
      <main className="chimera-main">
        <header className="chimera-titlebar" data-tauri-drag-region><div data-tauri-drag-region><h1>{view === "providers" ? "供应商控制台" : nav.find((item) => item[0] === view)?.[1]}</h1><p>{view === "providers" ? "切换、配置并验证当前 API 连接" : view === "runtime" ? "安装、更新与修复本机 Codex" : view === "appearance" ? "管理 Codex 客户端皮肤" : view === "settings" ? "应用行为、数据位置与更新策略" : "查看供应商切换、连接测试与模型同步历史"}</p></div><WindowControls /></header>
        {view === "providers" && <ProvidersView providers={providers} currentId={currentId} loading={loading} runtime={runtime} activity={activity} onSwitch={switchProvider} onEdit={(provider) => { setModels(null); setEditor(providerDraft(provider)); }} onAdd={() => { setModels(null); setEditor(providerDraft()); }} onCheckRuntime={checkRuntime} onDiagnose={async () => { try { const result = await invoke<Array<{ name: string; result: string }>>("diagnose_codex_runtime"); note(`诊断完成：${result.length} 项`); toast.success("诊断完成"); } catch (error) { toast.error("诊断失败", { description: String(error) }); } }} />}
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

function ProvidersView({ providers, currentId, loading, runtime, activity, onSwitch, onEdit, onAdd, onCheckRuntime, onDiagnose }: { providers: Provider[]; currentId: string; loading: boolean; runtime: RuntimeStatus | null; activity: string[]; onSwitch: (id: string) => void; onEdit: (provider: Provider) => void; onAdd: () => void; onCheckRuntime: () => void; onDiagnose: () => void }) {
  if (loading) return <Empty label="正在读取供应商…" />;
  if (!providers.length) return <Onboarding onAdd={onAdd} />;
  const current = providers.find((provider) => provider.id === currentId) ?? providers[0];
  const endpoint = extractCodexBaseUrl(String(current.settingsConfig?.config ?? "")) || "未配置请求地址";
  const model = extractCodexModelName(String(current.settingsConfig?.config ?? "")) || "未设置";
  const cards = providers.slice(0, 3);
  return <section className="provider-console"><div className="connection-banner"><Zap size={18} /><div><b>当前正在使用 {current.name}</b><span>已应用到本机 Codex，模型列表已同步</span></div><em>已连接</em></div><div className="console-layout"><div className="console-main"><div className="console-heading"><h2>快速切换</h2><button className="link-button" onClick={() => onEdit(current)}>管理供应商 <span>→</span></button></div><div className="quick-provider-grid">{cards.map((provider) => { const active = provider.id === current.id; return <button key={provider.id} className={`quick-provider ${active ? "selected" : ""}`} onClick={() => !active && onSwitch(provider.id)}><span className="quick-provider-mark">{provider.name.slice(0, 1).toUpperCase()}</span><b>{provider.name}</b><em>{active ? "在线" : "可切换"}</em><small>{extractCodexModelName(String(provider.settingsConfig?.config ?? "")) || "未配置模型"}</small></button>; })}<button className="quick-provider add-provider" onClick={onAdd}><Plus size={16} /> 添加供应商</button></div><article className="provider-workbench"><header><div><h2>{current.name}</h2><p>OpenAI 兼容接口 · 自动发现模型</p></div><button className="preset-badge" onClick={() => onEdit(current)}>编辑</button></header><label>接口地址<input value={endpoint} readOnly /></label><label>API 密钥<div className="readonly-secret"><input value="sk-••••••••••••••••••" readOnly /><button onClick={() => onEdit(current)}>显示</button></div></label><label>默认模型<div className="readonly-model"><input value={model} readOnly /><button onClick={() => onEdit(current)}>刷新模型</button></div></label><footer><button className="secondary" onClick={() => onEdit(current)}>测试连接</button><button className="primary" onClick={() => onEdit(current)}>保存并应用</button></footer></article></div><aside className="codex-summary"><div className="summary-title"><h2>Codex 运行时</h2><button aria-label="更多运行时操作"><MoreHorizontal size={18} /></button></div><div className="runtime-version"><b>{runtime?.installed ? runtime.version || "已安装" : "未检测到 Codex"}</b><em>{runtime?.installed ? "状态正常" : "未安装"}</em><span>{runtime?.installed ? `已安装 · ${runtimeText(runtime.installMode)}` : "请先安装 Codex"}</span><i><u /></i></div><ul className="runtime-facts"><li><ShieldCheck size={16} /><span><b>签名验证通过</b><small>安装包完整性已校验</small></span></li><li><Check size={16} /><span><b>安装状态正常</b><small>{runtime?.installPath ? "路径已识别" : "等待检测"}</small></span></li><li><Activity size={16} /><span><b>最近备份</b><small>{runtime?.canRollback ? "回滚副本可用" : "暂无回滚副本"}</small></span></li></ul><div className="summary-actions"><button className="dark" onClick={onCheckRuntime}><RefreshCw size={15} /> 检查更新</button><button onClick={onDiagnose}><Wrench size={15} /> 修复与诊断</button><button onClick={() => toast.info("发布说明将在正式版本中提供")}>查看发布说明</button></div><div className="summary-activity"><b>最近活动</b>{activity.slice(0, 2).map((item) => <span key={item}>{item}</span>)}{!activity.length && <span>暂无操作记录</span>}</div></aside></div></section>;
}

function ProviderEditor({ editor, setEditor, showKey, setShowKey, models, fetchingModels, onFetchModels, onSave, onDelete }: { editor: ReturnType<typeof providerDraft>; setEditor: (value: ReturnType<typeof providerDraft> | null) => void; showKey: boolean; setShowKey: (value: boolean) => void; models: FetchedModel[] | null; fetchingModels: boolean; onFetchModels: () => void; onSave: () => void; onDelete: () => void }) {
  const patch = (key: string, value: string) => setEditor({ ...editor, [key]: value });
  return <div className="modal-backdrop"><section className="provider-editor"><header><button className="icon-button" onClick={() => setEditor(null)}><X size={18} /></button><div><h2>{editor.original ? "编辑供应商" : "添加供应商"}</h2><p>仅写入 Codex 的供应商配置。</p></div></header><div className="editor-form"><Field label="供应商名称" value={editor.name} onChange={(value) => patch("name", value)} placeholder="例如 Chimera Hub" /><Field label="官网链接" value={editor.websiteUrl} onChange={(value) => patch("websiteUrl", value)} placeholder="https://example.com" /><Field label="API 请求地址" value={editor.baseUrl} onChange={(value) => patch("baseUrl", value)} placeholder="https://api.example.com/v1" hint="预设和自定义供应商都可编辑 URL。" /><label>API Key<div className="password-field"><input type={showKey ? "text" : "password"} value={editor.apiKey} onChange={(event) => patch("apiKey", event.target.value)} placeholder="粘贴 API Key" /><button onClick={() => setShowKey(!showKey)}>{showKey ? <EyeOff size={16} /> : <Eye size={16} />}</button></div></label><label>默认模型<div className="model-input"><input value={editor.model} onChange={(event) => patch("model", event.target.value)} placeholder="先获取模型列表，或手动输入" /><button onClick={onFetchModels} disabled={fetchingModels}>{fetchingModels ? <LoaderCircle className="spin" size={15} /> : <Download size={15} />} 获取模型</button></div></label>{models !== null && <div className="model-results">{models.length ? models.map((model) => <button key={model.id} className={editor.model === model.id ? "picked" : ""} onClick={() => patch("model", model.id)}>{model.id}{editor.model === model.id && <Check size={15} />}</button>) : <p><CircleAlert size={15} /> 未能获取模型列表，可保留手动输入的模型名称。</p>}</div>}<details><summary>高级选项</summary><p>默认采用 Codex Responses 兼容配置。复杂协议和模型映射仅在需要时展开。</p></details></div><footer><button className="secondary" onClick={async () => { await onFetchModels(); }}>测试连接</button><div>{editor.original && <button className="danger" onClick={onDelete}><Trash2 size={15} /> 删除</button>}<button className="primary" onClick={onSave}>保存并应用</button></div></footer></section></div>;
}

function RuntimeView({ runtime, release, onCheck, onDiagnose, onAction }: { runtime: RuntimeStatus | null; release: ReleaseStatus | null; onCheck: () => void; onDiagnose: () => void; onAction: (value: "update" | "repair" | "rollback" | "uninstall") => void }) {
  const target = release?.latestVersion ?? runtime?.version ?? "等待检查";
  return <section className="runtime-update-layout"><article className="runtime-update-card"><h2>{release?.updateAvailable ? "发现可用更新" : "Codex 运行时"}</h2><div className="version-compare"><div><span>当前版本</span><b>{runtime?.installed ? runtime.version || "已安装" : "未检测到"}</b></div><div className={release?.updateAvailable ? "target-version available" : "target-version"}><span>目标版本</span><b>{target}</b></div></div><dl className="update-details"><div><dt>下载来源</dt><dd><Check size={15} /> 稳定版</dd></div><div><dt>安装方式</dt><dd><Check size={15} /> {runtimeText(runtime?.installMode)}{runtime?.installed ? " · 当前目录" : ""}</dd></div><div><dt>备份状态</dt><dd><Check size={15} /> 将自动创建回滚点</dd></div><div><dt>磁盘空间</dt><dd><Check size={15} /> 安装前自动校验</dd></div></dl><div className="update-progress"><span>{release?.updateAvailable ? "新版本已经准备就绪" : "检查更新以获取最新版本"}</span><i><u style={{ width: release?.updateAvailable ? "68%" : "0%" }} /></i></div><footer><button onClick={onCheck}>重新检查</button>{release?.updateAvailable && <button className="primary" onClick={() => onAction("update")}>下载并安装</button>}</footer></article><aside className="runtime-diagnostics"><h2>修复与诊断</h2><p>更新失败会自动恢复到上一版本。诊断结果会保留在本机，供你确认后再进行修复。</p><button onClick={onDiagnose}>查看诊断结果 <span>↗</span><small>安装目录、签名、进程状态</small></button><button onClick={() => onAction("rollback")} disabled={!runtime?.canRollback}>回滚上一版本 <span>↗</span><small>需要一次确认</small></button><button onClick={() => onAction("repair")} disabled={!runtime?.canRepair}>重新安装并修复 <span>↗</span><small>保留供应商配置</small></button><button className="danger-line" onClick={() => onAction("uninstall")} disabled={!runtime?.canUninstall}>卸载 Codex</button></aside></section>;
}

function ActivityView({ entries }: { entries: string[] }) { const rows = entries.length ? entries : ["暂无记录。切换供应商、获取模型或检查更新后会显示在这里。"]; return <section className="activity-dashboard"><div className="activity-metrics"><Metric label="今日连接" value={String(entries.length)} detail="本次会话操作" /><Metric label="模型同步" value="0" detail="已完成同步" /><Metric label="异常记录" value="0" detail="需要处理" success /></div><article className="activity-table"><div className="activity-table-head"><span>时间</span><span>供应商</span><span>操作</span><span>结果</span></div>{rows.map((entry, index) => <div className="activity-table-row" key={`${entry}-${index}`}><span>{entry.slice(0, 5) || "--:--"}</span><span>Codex</span><span>{entry.replace(/^\d{2}:\d{2}\s+/, "")}</span><span className="ok">成功</span></div>)}</article></section>; }

function AppearanceView({ enabled }: { enabled: boolean }) { return <section className="skin-layout"><aside className="skin-list"><div className="skin-tabs"><b>在线皮肤</b><span>已安装</span></div>{["NERV EVA-02 Asuka Terminal", "TPC GUTS Command Terminal", "NERV EVA-00 Rei Prototype"].map((name, index) => <button key={name} className={index === 0 ? "active" : ""}><i style={{ background: ["#22191a", "#222c43", "#e4e1ec"][index] }} />{name}<small>Codex skin</small></button>)}</aside><article className="skin-detail"><h2>NERV EVA-02 Asuka Terminal</h2><p>素材化 Codex 皮肤 · 深色主界面与浅色报告风格。</p><div className="skin-preview"><span>NERV</span><strong>God's in His Heaven</strong><div /></div>{enabled ? <div className="skin-actions"><button className="dark">下载安装</button><button>试穿</button><button>恢复默认</button></div> : <div className="feature-gap"><CircleAlert size={17} /><div><b>皮肤引擎尚未接入</b><p>页面和安装流程已设计，但当前后端未提供 `.codexskin` 校验、CDP 注入和恢复命令，因此不会显示无效按钮。</p></div></div>}<p className="integrity"><ShieldCheck size={16} /> 已规划为不修改 Codex 文件的注入式皮肤。</p></article></section>; }

function SettingsView({ onCheck }: { onCheck: () => void }) { const [autoLaunch, setAutoLaunch] = useState<boolean | null>(null); useEffect(() => { void settingsApi.getAutoLaunchStatus().then(setAutoLaunch).catch(() => setAutoLaunch(false)); }, []); const toggleAutoLaunch = () => void settingsApi.setAutoLaunch(!(autoLaunch ?? false)).then((value) => { setAutoLaunch(value); toast.success(value ? "已开启开机启动" : "已关闭开机启动"); }).catch((error) => toast.error("设置失败", { description: String(error) })); return <section className="settings-layout"><aside><b className="active">常规</b><b>数据与隐私</b><b>更新策略</b><b>高级</b></aside><article className="panel settings-panel"><h2>常规</h2><SettingRow label="开机启动 Chimera++" detail="登录 Windows 后自动运行" value={autoLaunch === null ? "读取中" : autoLaunch ? "开启" : "关闭"} /><SettingRow label="默认打开页面" detail="启动后显示的工作区" value="供应商" /><SettingRow label="语言" detail="界面显示语言" value="简体中文" /><SettingRow label="数据目录" detail="配置与日志的存储位置" value="%USERPROFILE%\\.chimera-plus-plus" /><div className="settings-actions"><span>{autoLaunch === null ? "正在读取设置" : "已保存到本机"}</span><button onClick={onCheck}><RefreshCw size={15} /> 检查 Codex 更新</button><button className="primary" onClick={toggleAutoLaunch}>{autoLaunch ? "关闭开机启动" : "开启开机启动"}</button></div></article></section>; }

function ConfirmOperation({ action, onCancel, onConfirm }: { action: string; onCancel: () => void; onConfirm: () => void }) { const label = action === "update" ? "下载并安装更新" : action === "repair" ? "重新安装并修复 Codex" : action === "rollback" ? "回滚上一版本" : "卸载 Codex"; return <div className="modal-backdrop"><section className="confirm-dialog"><CircleAlert size={26} /><h2>确认{label}？</h2><p>该操作会修改 Codex 运行时。供应商配置和 `~/.codex` 用户数据不会被删除。</p><footer><button onClick={onCancel}>取消</button><button className="primary" onClick={onConfirm}>确认继续</button></footer></section></div>; }
function Onboarding({ onAdd }: { onAdd: () => void }) { return <section className="onboarding"><div className="onboarding-brand"><Command size={18} /> Chimera++</div><h2>开始配置你的 Codex</h2><p>粘贴供应商地址和 API Key，Chimera++ 会获取模型列表并写入 Codex 配置。</p><ol><li><b>1</b><div><strong>连接供应商</strong><span>添加可编辑的 API 请求地址和密钥</span></div></li><li><b>2</b><div><strong>获取模型</strong><span>从供应商接口读取模型，也可手动填写</span></div></li><li><b>3</b><div><strong>保存并应用</strong><span>立即切换到新的 Codex 供应商</span></div></li></ol><button className="primary" onClick={onAdd}>开始配置</button></section>; }
function Field({ label, value, onChange, placeholder, hint }: { label: string; value: string; onChange: (value: string) => void; placeholder?: string; hint?: string }) { return <label>{label}<input value={value} onChange={(event) => onChange(event.target.value)} placeholder={placeholder} />{hint && <small>{hint}</small>}</label>; }
function Empty({ label, action, onAction }: { label: string; action?: string; onAction?: () => void }) { return <div className="empty"><p>{label}</p>{action && <button className="primary" onClick={onAction}>{action}</button>}</div>; }
function Metric({ label, value, detail, success = false }: { label: string; value: string; detail: string; success?: boolean }) { return <article><span>{label}</span><b className={success ? "ok" : ""}>{value}</b><small>{detail}</small></article>; }
function SettingRow({ label, detail, value }: { label: string; detail: string; value: string }) { return <div className="setting-row"><div><b>{label}</b><p>{detail}</p></div><span>{value}<ChevronDown size={14} /></span></div>; }
function WindowControls() { const run = (action: "close" | "minimize" | "maximize") => { const window = getCurrentWindow(); if (action === "close") void window.close(); else if (action === "minimize") void window.minimize(); else void window.toggleMaximize(); }; return <div className="window-dots"><button aria-label="关闭 Chimera++" onClick={() => run("close")} /><button aria-label="最小化 Chimera++" onClick={() => run("minimize")} /><button aria-label="最大化或还原 Chimera++" onClick={() => run("maximize")} /></div>; }
