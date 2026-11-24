export interface YsmModelEntry {
  persona_index: number;
  file_name: string;
  display_name: string;
  size_bytes: number;
}

export interface YsmActiveState {
  persona_index: number;
  gesture_index: number;
  active_persona: string;
  title: string;
}

export interface YsmOverview {
  active: YsmActiveState;
  models: YsmModelEntry[];
  desktop_models: YsmModelEntry[];
}

export interface SchematicEntry {
  index: number;
  file_name: string;
  size_bytes: number;
  is_active: boolean;
}

export interface SchematicRenderOpts {
  layer_mode: number;
  mirror: boolean;
  ghost_render: boolean;
}

export interface SchematicTargetInfo {
  file: string;
  name: string;
  category: string;
  total_blocks: number;
}

export interface SchematicsOverview {
  active_index: number;
  target: SchematicTargetInfo;
  render_opts: SchematicRenderOpts;
  items: SchematicEntry[];
}

export interface ModEntry {
  id: string;
  file_name: string;
  enabled: boolean;
  size_bytes: number;
}

export interface LocaleBundle {
  meta: { code: string; label: string };
  gui: Record<string, string>;
  mods: Record<string, Record<string, string>>;
}

export interface SyncReport {
  active_locale: string;
  synced_mods: string[];
  written_files: number;
}

export interface HealthResponse {
  status: string;
  service: string;
  default_locale: string;
  horizon_dir: string;
}
