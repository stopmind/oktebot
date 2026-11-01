use crate::{login::Login, oknoid::OknoId};
use axum::{Router, routing::*};
use log::{error, info};
use std::sync::Arc;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub mod login;

pub struct ApiContext {
    pub db: Arc<OknoId>,
    pub login: Arc<Login>,
}

type ApiCtx = Arc<ApiContext>;

pub struct ApiError(anyhow::Error);

impl<E> From<E> for ApiError
where
    E: Into<anyhow::Error>
{
    fn from(err: E) -> Self {
        ApiError(err.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        error!("{}", self.0);
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

type ApiResult<T = Response> = Result<T, ApiError>;

pub async fn serve_api(db: Arc<OknoId>, login: Arc<Login>) -> anyhow::Result<()> {
    info!("starting api server...");

    let ctx = Arc::new(ApiContext { db, login });

    let app = Router::new()
        .route("/v1/auth/{token}", get(login::auth))
        .route("/v1/auth/{token}/ban/{user}", post(login::ban))
        .route("/v1/events", any(login::events_ws))
        .with_state(ctx);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8888").await?;
    axum::serve(listener, app).await?;
    Ok(())
}
