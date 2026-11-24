mod api_error;
mod routes;

use std::path::PathBuf;
use std::sync::Arc;

use horizon_core::config::StudioConfig;
use horizon_core::horizon_fs::HorizonStore;
use horizon_i18n::I18nStore;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;

use crate::routes::{build_router, AppState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg_path = resolve_config_path();
    let cfg = StudioConfig::load(&cfg_path)?;
    let store = HorizonStore::new(cfg.paths.horizon_dir, cfg.paths.extra_ysm_dir);
    let i18n = I18nStore::new(cfg.paths.locales_dir, cfg.i18n.supported);
    let state = Arc::new(AppState {
        store,
        i18n,
        default_locale: cfg.i18n.default_locale,
    });

    let app = build_router(state)
        .layer(CorsLayer::permissive())
        .layer(CompressionLayer::new());

    let listener = tokio::net::TcpListener::bind(&cfg.server.bind_addr).await?;
    println!(
        "lilyco-horizon-daemon listening on http://{}",
        cfg.server.bind_addr
    );
    axum::serve(listener, app).await?;
    Ok(())
}

fn resolve_config_path() -> PathBuf {
    let args: Vec<String> = std::env::args().skip(1).collect();
    for pair in args.windows(2) {
        if pair[0] == "--config" {
            return PathBuf::from(&pair[1]);
        }
    }
    PathBuf::from("/home/lilyco/项目/lilyco-horizon/config/studio.toml")
}
