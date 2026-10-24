use crate::{login::Login, oknoid::OknoId};
use anyhow::Result;
use axum::{Router, routing::get};
use log::info;
use std::sync::Arc;

pub mod login;

pub struct ApiContext {
    pub db: Arc<OknoId>,
    pub login: Arc<Login>,
}

type ApiCtx = Arc<ApiContext>;

pub async fn serve_api(db: Arc<OknoId>, login: Arc<Login>) -> Result<()> {
    info!("starting api server...");

    let ctx = Arc::new(ApiContext { db, login });

    let app = Router::new()
        .route("/v1/auth/{token}", get(login::auth))
        .with_state(ctx);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8888").await?;
    axum::serve(listener, app).await?;
    Ok(())
}
