import { useCallback, useEffect, useState } from "react";
import {
  Blocks,
  Box,
  CheckCircle2,
  Download,
  Globe,
  Languages,
  Layers,
  Puzzle,
  RefreshCw,
  Sparkles,
  UserRound,
} from "lucide-react";
import { api } from "./api";
import type {
  HealthResponse,
  LocaleBundle,
  ModEntry,
  SchematicsOverview,
  YsmOverview,
} from "./types";

type TabId = "ysm" | "schematics" | "mods" | "i18n";

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

export function App() {
  const [tab, setTab] = useState<TabId>("ysm");
  const [locale, setLocale] = useState<string>("zh-CN");
  const [bundle, setBundle] = useState<LocaleBundle | null>(null);
  const [health, setHealth] = useState<HealthResponse | null>(null);
  const [ysm, setYsm] = useState<YsmOverview | null>(null);
  const [schematics, setSchematics] = useState<SchematicsOverview | null>(null);
  const [mods, setMods] = useState<ModEntry[]>([]);
  const [toast, setToast] = useState<string>("");
  const [busy, setBusy] = useState<boolean>(false);

  const t = useCallback(
    (key: string, fallback: string) => bundle?.gui[key] ?? fallback,
    [bundle]
  );

  const notify = (msg: string) => {
    setToast(msg);
    window.setTimeout(() => setToast(""), 3500);
  };

  const refreshAll = useCallback(async (targetLocale: string) => {
    setBusy(true);
    try {
      const [h, b, y, s, m] = await Promise.all([
        api.health(),
        api.getLocaleBundle(targetLocale),
        api.getYsm(),
        api.getSchematics(),
        api.getMods(),
      ]);
      setHealth(h);
      setBundle(b);
      setYsm(y);
      setSchematics(s);
      setMods(m);
    } catch (err) {
      notify(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    void refreshAll(locale);
  }, [locale, refreshAll]);

  const handleSwitchYsm = async (idx: number, title: string) => {
    const gesture = ysm?.active.gesture_index ?? 0;
    const active = await api.switchYsm(idx, gesture, title);
    setYsm((prev) => (prev ? { ...prev, active } : prev));
    notify(`YSM -> ${title}`);
  };

  const handleImportYsm = async (fileName: string) => {
    await api.importDesktopYsm(fileName);
    await refreshAll(locale);
    notify(`Imported ${fileName}`);
  };

  const handleSwitchSchematic = async (fileName: string, index: number) => {
    const opts = schematics?.render_opts ?? {
      layer_mode: 0,
      mirror: false,
      ghost_render: true,
    };
    const next = await api.switchSchematic(fileName, index, opts);
    setSchematics(next);
    notify(`Schematic -> ${fileName}`);
  };

  const handleUpdateRenderOpts = async (patch: Partial<SchematicsOverview["render_opts"]>) => {
    if (!schematics) return;
    const activeFile = schematics.target.file || schematics.items[0]?.file_name;
    if (!activeFile) return;
    const next = await api.switchSchematic(activeFile, schematics.active_index, {
      ...schematics.render_opts,
      ...patch,
    });
    setSchematics(next);
  };

  const handleToggleMod = async (id: string, enabled: boolean) => {
    const next = await api.toggleMod(id, enabled);
    setMods(next);
    notify(`${id} -> ${enabled ? "ON" : "OFF"}`);
  };

  const handleUpdateI18n = async (modId: string, key: string, value: string) => {
    const next = await api.updateI18nEntry(locale, modId, key, value);
    setBundle(next);
    notify(`Saved ${modId}.${key}`);
  };

  const handleSyncI18n = async () => {
    const report = await api.syncModI18n(locale);
    notify(
      `Synced ${report.active_locale} -> ${report.synced_mods.length} mods (${report.written_files} files)`
    );
  };

  return (
    <div className="min-h-screen bg-slate-950 text-slate-100">
      <header className="border-b border-slate-800 bg-slate-900/80 backdrop-blur">
        <div className="mx-auto flex max-w-6xl flex-wrap items-center justify-between gap-4 px-6 py-4">
          <div className="flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-sky-500/20 text-sky-400">
              <Sparkles className="h-5 w-5" />
            </div>
            <div>
              <h1 className="text-lg font-semibold tracking-tight">
                {t("app_title", "Lilyco Horizon Studio")}
              </h1>
              <p className="text-xs text-slate-400">
                {t("app_subtitle", "YSM Models · Litematica Schematics · Mods & i18n")}
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2">
            <span
              data-testid="daemon-status"
              className={`inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-xs font-medium ${
                health
                  ? "bg-emerald-500/15 text-emerald-300"
                  : "bg-rose-500/15 text-rose-300"
              }`}
            >
              <span
                className={`h-2 w-2 rounded-full ${health ? "bg-emerald-400" : "bg-rose-400"}`}
              />
              {health ? t("status_online", "Online") : t("status_offline", "Offline")}
            </span>

            <button
              data-testid="toggle-locale"
              onClick={() => setLocale((prev) => (prev === "zh-CN" ? "en-US" : "zh-CN"))}
              className="inline-flex items-center gap-1.5 rounded-lg border border-slate-700 bg-slate-800 px-3 py-1.5 text-xs font-medium hover:bg-slate-700"
            >
              <Globe className="h-3.5 w-3.5 text-sky-400" />
              {locale === "zh-CN" ? "简体中文" : "English"}
            </button>

            <button
              data-testid="refresh-all"
              onClick={() => void refreshAll(locale)}
              disabled={busy}
              className="inline-flex items-center gap-1.5 rounded-lg border border-slate-700 bg-slate-800 px-3 py-1.5 text-xs font-medium hover:bg-slate-700 disabled:opacity-50"
            >
              <RefreshCw className={`h-3.5 w-3.5 ${busy ? "animate-spin" : ""}`} />
              {t("btn_refresh", "Refresh")}
            </button>
          </div>
        </div>

        <nav className="mx-auto flex max-w-6xl gap-2 px-6 pb-3">
          {(
            [
              ["ysm", t("tab_ysm", "YSM Models"), UserRound],
              ["schematics", t("tab_schematics", "Schematics"), Blocks],
              ["mods", t("tab_mods", "Mods"), Puzzle],
              ["i18n", t("tab_i18n", "Mod i18n"), Languages],
            ] as const
          ).map(([id, label, Icon]) => (
            <button
              key={id}
              data-testid={`tab-${id}`}
              onClick={() => setTab(id)}
              className={`inline-flex items-center gap-2 rounded-lg px-4 py-2 text-sm font-medium transition ${
                tab === id
                  ? "bg-sky-500 text-slate-950 shadow-sm"
                  : "text-slate-300 hover:bg-slate-800 hover:text-white"
              }`}
            >
              <Icon className="h-4 w-4" />
              {label}
            </button>
          ))}
        </nav>
      </header>

      <main className="mx-auto max-w-6xl px-6 py-6">
        {toast && (
          <div
            data-testid="toast-banner"
            className="mb-4 flex items-center gap-2 rounded-xl border border-sky-500/40 bg-sky-500/10 px-4 py-2.5 text-sm text-sky-200"
          >
            <CheckCircle2 className="h-4 w-4 text-sky-400" />
            {toast}
          </div>
        )}

        {tab === "ysm" && ysm && (
          <section className="space-y-6" data-testid="panel-ysm">
            <div className="rounded-2xl border border-slate-800 bg-slate-900 p-5">
              <div className="text-xs uppercase tracking-wider text-sky-400">
                {t("label_active", "Active")} YSM
              </div>
              <div className="mt-1 text-xl font-semibold">{ysm.active.title}</div>
              <div className="mt-2 flex flex-wrap gap-3 text-xs text-slate-400">
                <span>Preset #{ysm.active.persona_index}</span>
                <span>Gesture #{ysm.active.gesture_index}</span>
                <span>ID: {ysm.active.active_persona}</span>
              </div>
            </div>

            {ysm.desktop_models.length > 0 && (
              <div className="rounded-2xl border border-amber-500/30 bg-amber-500/5 p-5">
                <h2 className="mb-3 text-sm font-semibold text-amber-300">
                  {t("btn_import_lyco", "Desktop YSM Staging")}
                </h2>
                <div className="flex flex-wrap gap-3">
                  {ysm.desktop_models.map((item) => (
                    <button
                      key={item.file_name}
                      data-testid={`import-ysm-${item.file_name}`}
                      onClick={() => void handleImportYsm(item.file_name)}
                      className="inline-flex items-center gap-2 rounded-xl border border-amber-400/40 bg-slate-900 px-4 py-2 text-sm hover:bg-amber-500/20"
                    >
                      <Download className="h-4 w-4 text-amber-300" />
                      <span>{item.file_name}</span>
                      <span className="text-xs text-slate-400">{formatBytes(item.size_bytes)}</span>
                    </button>
                  ))}
                </div>
              </div>
            )}

            <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
              {ysm.models.map((m) => {
                const isActive = ysm.active.persona_index === m.persona_index;
                return (
                  <div
                    key={m.file_name}
                    className={`flex flex-col justify-between rounded-2xl border p-4 transition ${
                      isActive
                        ? "border-sky-500 bg-sky-500/10"
                        : "border-slate-800 bg-slate-900 hover:border-slate-700"
                    }`}
                  >
                    <div>
                      <div className="flex items-center justify-between gap-2">
                        <span className="inline-flex items-center gap-1.5 text-sm font-semibold">
                          <Box className="h-4 w-4 text-sky-400" />
                          {m.display_name}
                        </span>
                        <span className="text-xs text-slate-400">
                          #{m.persona_index} · {formatBytes(m.size_bytes)}
                        </span>
                      </div>
                      <p className="mt-1 truncate text-xs text-slate-500">{m.file_name}</p>
                    </div>
                    <button
                      data-testid={`activate-ysm-${m.persona_index}`}
                      onClick={() => void handleSwitchYsm(m.persona_index, m.display_name)}
                      className={`mt-4 w-full rounded-lg py-2 text-xs font-semibold ${
                        isActive
                          ? "bg-sky-500 text-slate-950"
                          : "bg-slate-800 text-slate-200 hover:bg-slate-700"
                      }`}
                    >
                      {isActive ? t("label_active", "Active") : t("btn_apply", "Apply")}
                    </button>
                  </div>
                );
              })}
            </div>
          </section>
        )}

        {tab === "schematics" && schematics && (
          <section className="space-y-6" data-testid="panel-schematics">
            <div className="flex flex-wrap items-center justify-between gap-4 rounded-2xl border border-slate-800 bg-slate-900 p-5">
              <div>
                <div className="text-xs uppercase tracking-wider text-emerald-400">
                  {t("label_active", "Active")} Litematica
                </div>
                <div className="mt-1 text-xl font-semibold">{schematics.target.name}</div>
                <div className="mt-1 text-xs text-slate-400">
                  {schematics.target.file} · {schematics.target.category}
                </div>
              </div>

              <div className="flex flex-wrap items-center gap-2">
                <select
                  data-testid="schematic-layer-mode"
                  value={schematics.render_opts.layer_mode}
                  onChange={(e) =>
                    void handleUpdateRenderOpts({ layer_mode: Number(e.target.value) })
                  }
                  className="rounded-lg border border-slate-700 bg-slate-800 px-3 py-1.5 text-xs"
                >
                  <option value={0}>All Layers (完整3D)</option>
                  <option value={1}>Single Layer (单层切片)</option>
                  <option value={2}>All Below (当前层及以下)</option>
                </select>
                <button
                  data-testid="schematic-toggle-mirror"
                  onClick={() =>
                    void handleUpdateRenderOpts({ mirror: !schematics.render_opts.mirror })
                  }
                  className={`rounded-lg px-3 py-1.5 text-xs font-medium ${
                    schematics.render_opts.mirror
                      ? "bg-sky-500 text-slate-950"
                      : "bg-slate-800 text-slate-300"
                  }`}
                >
                  Mirror: {schematics.render_opts.mirror ? "ON" : "OFF"}
                </button>
                <button
                  data-testid="schematic-toggle-ghost"
                  onClick={() =>
                    void handleUpdateRenderOpts({
                      ghost_render: !schematics.render_opts.ghost_render,
                    })
                  }
                  className={`rounded-lg px-3 py-1.5 text-xs font-medium ${
                    schematics.render_opts.ghost_render
                      ? "bg-emerald-500 text-slate-950"
                      : "bg-slate-800 text-slate-300"
                  }`}
                >
                  Ghost: {schematics.render_opts.ghost_render ? "ON" : "OFF"}
                </button>
              </div>
            </div>

            <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
              {schematics.items.map((item) => (
                <div
                  key={item.file_name}
                  className={`flex flex-col justify-between rounded-2xl border p-4 ${
                    item.is_active
                      ? "border-emerald-500 bg-emerald-500/10"
                      : "border-slate-800 bg-slate-900"
                  }`}
                >
                  <div>
                    <div className="flex items-center justify-between gap-2">
                      <span className="inline-flex items-center gap-1.5 text-sm font-semibold">
                        <Layers className="h-4 w-4 text-emerald-400" />
                        {item.file_name.replace(".litematic", "")}
                      </span>
                      <span className="text-xs text-slate-400">
                        {formatBytes(item.size_bytes)}
                      </span>
                    </div>
                    <p className="mt-1 text-xs text-slate-500">Index #{item.index}</p>
                  </div>
                  <button
                    data-testid={`activate-schematic-${item.index}`}
                    onClick={() => void handleSwitchSchematic(item.file_name, item.index)}
                    className={`mt-4 w-full rounded-lg py-2 text-xs font-semibold ${
                      item.is_active
                        ? "bg-emerald-500 text-slate-950"
                        : "bg-slate-800 text-slate-200 hover:bg-slate-700"
                    }`}
                  >
                    {item.is_active ? t("label_active", "Active") : t("btn_apply", "Apply")}
                  </button>
                </div>
              ))}
            </div>
          </section>
        )}

        {tab === "mods" && (
          <section className="grid gap-4 sm:grid-cols-2" data-testid="panel-mods">
            {mods.map((m) => (
              <div
                key={m.id}
                className="flex items-center justify-between rounded-2xl border border-slate-800 bg-slate-900 p-4"
              >
                <div>
                  <div className="text-sm font-semibold">{m.id}</div>
                  <div className="text-xs text-slate-400">
                    {m.file_name} · {formatBytes(m.size_bytes)}
                  </div>
                </div>
                <button
                  data-testid={`toggle-mod-${m.id}`}
                  onClick={() => void handleToggleMod(m.id, !m.enabled)}
                  className={`rounded-lg px-4 py-2 text-xs font-semibold ${
                    m.enabled
                      ? "bg-emerald-500/20 text-emerald-300 ring-1 ring-emerald-500/40"
                      : "bg-slate-800 text-slate-400"
                  }`}
                >
                  {m.enabled ? t("label_enabled", "Enabled") : t("label_disabled", "Disabled")}
                </button>
              </div>
            ))}
          </section>
        )}

        {tab === "i18n" && bundle && (
          <section className="space-y-5" data-testid="panel-i18n">
            <div className="flex flex-wrap items-center justify-between gap-4 rounded-2xl border border-slate-800 bg-slate-900 p-5">
              <div>
                <h2 className="text-base font-semibold">
                  {bundle.meta.label} ({bundle.meta.code})
                </h2>
                <p className="text-xs text-slate-400">
                  locales/{bundle.meta.code}.toml → Horizon Mod JSON + lilyco_i18n
                </p>
              </div>
              <button
                data-testid="sync-mod-i18n"
                onClick={() => void handleSyncI18n()}
                className="rounded-xl bg-sky-500 px-4 py-2.5 text-sm font-semibold text-slate-950 hover:bg-sky-400"
              >
                {t("btn_sync_i18n", "Sync Mod Locales")}
              </button>
            </div>

            {Object.entries(bundle.mods).map(([modId, entries]) => (
              <div
                key={modId}
                className="rounded-2xl border border-slate-800 bg-slate-900 p-5"
              >
                <h3 className="mb-3 text-sm font-semibold text-sky-400">[{modId}]</h3>
                <div className="space-y-2.5">
                  {Object.entries(entries).map(([k, v]) => (
                    <div key={k} className="grid gap-2 sm:grid-cols-12 sm:items-center">
                      <code className="truncate text-xs text-slate-400 sm:col-span-5">{k}</code>
                      <input
                        defaultValue={v}
                        onBlur={(e) => {
                          if (e.target.value !== v) {
                            void handleUpdateI18n(modId, k, e.target.value);
                          }
                        }}
                        className="rounded-lg border border-slate-700 bg-slate-950 px-3 py-1.5 text-sm text-slate-100 focus:border-sky-500 focus:outline-none sm:col-span-7"
                      />
                    </div>
                  ))}
                </div>
              </div>
            ))}
          </section>
        )}
      </main>
    </div>
  );
}
