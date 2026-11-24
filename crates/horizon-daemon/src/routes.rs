use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use horizon_core::horizon_fs::HorizonStore;
use horizon_core::models::{
    ModEntry, ModToggleRequest, SchematicSwitchRequest, SchematicsOverview, YsmActiveState,
    YsmImportRequest, YsmModelEntry, YsmOverview, YsmSwitchRequest,
};
use horizon_i18n::{I18nStore, LocaleBundle, SyncReport, UpdateModEntryRequest};
use serde::{Deserialize, Serialize};

use crate::api_error::ApiError;

pub struct AppState {
    pub store: HorizonStore,
    pub i18n: I18nStore,
    pub default_locale: String,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    service: String,
    default_locale: String,
    horizon_dir: String,
}

#[derive(Serialize)]
struct AiEndpointSpec {
    method: String,
    path: String,
    summary: String,
}

#[derive(Serialize)]
struct AiCapabilitiesResponse {
    protocol: String,
    endpoints: Vec<AiEndpointSpec>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SyncLocaleRequest {
    locale: String,
}

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/ai/capabilities", get(ai_capabilities))
        .route("/api/v1/ysm", get(get_ysm))
        .route("/api/v1/ysm/switch", post(post_ysm_switch))
        .route("/api/v1/ysm/import", post(post_ysm_import))
        .route("/api/v1/schematics", get(get_schematics))
        .route("/api/v1/schematics/switch", post(post_schematic_switch))
        .route("/api/v1/mods", get(get_mods))
        .route("/api/v1/mods/toggle", post(post_mod_toggle))
        .route("/api/v1/i18n/locales", get(get_locales))
        .route("/api/v1/i18n/{locale}", get(get_locale_bundle))
        .route("/api/v1/i18n/entry", post(post_i18n_entry))
        .route("/api/v1/i18n/sync-mods", post(post_i18n_sync))
        .with_state(state)
}

async fn health(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        service: "lilyco-horizon-daemon".to_string(),
        default_locale: state.default_locale.clone(),
        horizon_dir: state.store.horizon_dir().display().to_string(),
    })
}

async fn ai_capabilities() -> Json<AiCapabilitiesResponse> {
    let specs = vec![
        ep("GET", "/api/v1/ysm", "List Horizon & desktop YSM models and active state"),
        ep("POST", "/api/v1/ysm/switch", "Switch active YSM persona & gesture index"),
        ep("POST", "/api/v1/ysm/import", "Import a desktop .zip/.ysm model into Horizon"),
        ep("GET", "/api/v1/schematics", "List Litematica .litematic files & active target"),
        ep("POST", "/api/v1/schematics/switch", "Set active schematic & render options"),
        ep("GET", "/api/v1/mods", "List installed Horizon .hmod packages"),
        ep("POST", "/api/v1/mods/toggle", "Enable or disable a Horizon mod"),
        ep("GET", "/api/v1/i18n/{locale}", "Read external TOML locale bundle"),
        ep("POST", "/api/v1/i18n/entry", "Update a mod translation key in TOML"),
        ep("POST", "/api/v1/i18n/sync-mods", "Sync TOML translations into Horizon mods"),
    ];
    Json(AiCapabilitiesResponse {
        protocol: "lilyco-horizon-rest-v1".to_string(),
        endpoints: specs,
    })
}

fn ep(method: &str, path: &str, summary: &str) -> AiEndpointSpec {
    AiEndpointSpec {
        method: method.to_string(),
        path: path.to_string(),
        summary: summary.to_string(),
    }
}

async fn get_ysm(State(state): State<Arc<AppState>>) -> Result<Json<YsmOverview>, ApiError> {
    Ok(Json(state.store.load_ysm_overview()?))
}

async fn post_ysm_switch(
    State(state): State<Arc<AppState>>,
    Json(req): Json<YsmSwitchRequest>,
) -> Result<Json<YsmActiveState>, ApiError> {
    Ok(Json(state.store.switch_ysm(&req)?))
}

async fn post_ysm_import(
    State(state): State<Arc<AppState>>,
    Json(req): Json<YsmImportRequest>,
) -> Result<Json<YsmModelEntry>, ApiError> {
    Ok(Json(state.store.import_desktop_ysm(&req.file_name)?))
}

async fn get_schematics(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SchematicsOverview>, ApiError> {
    Ok(Json(state.store.load_schematics_overview()?))
}

async fn post_schematic_switch(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SchematicSwitchRequest>,
) -> Result<Json<SchematicsOverview>, ApiError> {
    Ok(Json(state.store.switch_schematic(&req)?))
}

async fn get_mods(State(state): State<Arc<AppState>>) -> Result<Json<Vec<ModEntry>>, ApiError> {
    Ok(Json(state.store.list_mods()?))
}

async fn post_mod_toggle(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ModToggleRequest>,
) -> Result<Json<Vec<ModEntry>>, ApiError> {
    Ok(Json(state.store.set_mod_enabled(&req.id, req.enabled)?))
}

async fn get_locales(State(state): State<Arc<AppState>>) -> Json<Vec<String>> {
    Json(state.i18n.supported_locales().to_vec())
}

async fn get_locale_bundle(
    State(state): State<Arc<AppState>>,
    Path(locale): Path<String>,
) -> Result<Json<LocaleBundle>, ApiError> {
    Ok(Json(state.i18n.load(&locale)?))
}

async fn post_i18n_entry(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpdateModEntryRequest>,
) -> Result<Json<LocaleBundle>, ApiError> {
    Ok(Json(state.i18n.update_mod_entry(&req)?))
}

async fn post_i18n_sync(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SyncLocaleRequest>,
) -> Result<Json<SyncReport>, ApiError> {
    let report = state
        .i18n
        .sync_to_horizon(state.store.horizon_dir(), &req.locale)?;
    Ok(Json(report))
}
