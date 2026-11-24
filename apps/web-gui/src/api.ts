import type {
  HealthResponse,
  LocaleBundle,
  ModEntry,
  SchematicRenderOpts,
  SchematicsOverview,
  SyncReport,
  YsmActiveState,
  YsmModelEntry,
  YsmOverview,
} from "./types";

async function requestJson<T>(url: string, init?: RequestInit): Promise<T> {
  const res = await fetch(url, {
    headers: { "Content-Type": "application/json" },
    ...init,
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`HTTP ${res.status}: ${text}`);
  }
  return (await res.json()) as T;
}

export const api = {
  health: () => requestJson<HealthResponse>("/api/v1/health"),
  getYsm: () => requestJson<YsmOverview>("/api/v1/ysm"),
  switchYsm: (persona_index: number, gesture_index: number, title: string) =>
    requestJson<YsmActiveState>("/api/v1/ysm/switch", {
      method: "POST",
      body: JSON.stringify({ persona_index, gesture_index, title }),
    }),
  importDesktopYsm: (file_name: string) =>
    requestJson<YsmModelEntry>("/api/v1/ysm/import", {
      method: "POST",
      body: JSON.stringify({ file_name }),
    }),
  getSchematics: () => requestJson<SchematicsOverview>("/api/v1/schematics"),
  switchSchematic: (file_name: string, index: number, render_opts: SchematicRenderOpts) =>
    requestJson<SchematicsOverview>("/api/v1/schematics/switch", {
      method: "POST",
      body: JSON.stringify({ file_name, index, render_opts }),
    }),
  getMods: () => requestJson<ModEntry[]>("/api/v1/mods"),
  toggleMod: (id: string, enabled: boolean) =>
    requestJson<ModEntry[]>("/api/v1/mods/toggle", {
      method: "POST",
      body: JSON.stringify({ id, enabled }),
    }),
  getLocaleBundle: (locale: string) => requestJson<LocaleBundle>(`/api/v1/i18n/${locale}`),
  updateI18nEntry: (locale: string, mod_id: string, key: string, value: string) =>
    requestJson<LocaleBundle>("/api/v1/i18n/entry", {
      method: "POST",
      body: JSON.stringify({ locale, mod_id, key, value }),
    }),
  syncModI18n: (locale: string) =>
    requestJson<SyncReport>("/api/v1/i18n/sync-mods", {
      method: "POST",
      body: JSON.stringify({ locale }),
    }),
};
