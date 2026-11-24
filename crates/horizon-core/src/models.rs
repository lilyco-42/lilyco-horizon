use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct YsmModelEntry {
    pub persona_index: u32,
    pub file_name: String,
    pub display_name: String,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct YsmActiveState {
    pub persona_index: u32,
    pub gesture_index: u32,
    pub active_persona: String,
    pub title: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct YsmOverview {
    pub active: YsmActiveState,
    pub models: Vec<YsmModelEntry>,
    pub desktop_models: Vec<YsmModelEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YsmSwitchRequest {
    pub persona_index: u32,
    pub gesture_index: u32,
    pub title: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YsmImportRequest {
    pub file_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SchematicEntry {
    pub index: u32,
    pub file_name: String,
    pub size_bytes: u64,
    pub is_active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SchematicRenderOpts {
    pub layer_mode: u32,
    pub mirror: bool,
    pub ghost_render: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SchematicTargetInfo {
    pub file: String,
    pub name: String,
    pub category: String,
    pub total_blocks: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SchematicsOverview {
    pub active_index: u32,
    pub target: SchematicTargetInfo,
    pub render_opts: SchematicRenderOpts,
    pub items: Vec<SchematicEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchematicSwitchRequest {
    pub file_name: String,
    pub index: u32,
    pub render_opts: SchematicRenderOpts,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModEntry {
    pub id: String,
    pub file_name: String,
    pub enabled: bool,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModToggleRequest {
    pub id: String,
    pub enabled: bool,
}
