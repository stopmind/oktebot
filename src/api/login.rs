use crate::api::{ApiCtx, ApiError, ApiResult};
use axum::{
    Json,
    extract::{Path, State},
    response::{IntoResponse, Response},
};
use axum::extract::WebSocketUpgrade;
use axum::extract::ws::{Message, WebSocket};
use futures::{Sink, SinkExt};
use log::{error, warn};
use reqwest::StatusCode;
use teloxide::types::UserId;
use tokio::sync::broadcast::error::RecvError;
use crate::oknoid::IdEvent;

pub async fn auth(State(ctx): State<ApiCtx>, Path(token): Path<String>) -> Response {
    match ctx.login.check_token(token.as_str()) {
        Ok(user) => match ctx.db.get_user_info(user).await {
            Ok(user) => (StatusCode::OK, Json(user)).into_response(),
            Err(err) => {
                error!("login failed: {err}");
                StatusCode::UNAUTHORIZED.into_response()
            }
        },
        Err(err) => {
            error!("token validation error: {err}");
            StatusCode::UNAUTHORIZED.into_response()
        }
    }
}

pub async fn ban(
    State(ctx): State<ApiCtx>,
    Path((token, user)): Path<(String, UserId)>
) -> ApiResult {
    let Ok(admin) = ctx.login.check_token(token.as_str()) else {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    };

    if !ctx.db.check_user_privileges(admin).await? {
        return Ok(StatusCode::FORBIDDEN.into_response());
    };

    ctx.db.set_user_banned(user, true).await?;
    Ok(StatusCode::OK.into_response())
}

pub async fn events_ws(
    State(ctx): State<ApiCtx>,
    upgrade: WebSocketUpgrade
) -> Response {
    upgrade.on_upgrade(|mut socket| async move {
        let mut receiver = ctx.db.get_events_receiver();

        loop {
            match receiver.recv().await {
                Ok(event) => {
                    if let Err(err) = send_event(&mut socket, event).await {
                        error!("failed to send event: {err}");
                    }
                }
                Err(RecvError::Lagged(count)) => {
                    warn!("websocket connection dropped {count} events");
                },
                Err(RecvError::Closed) => {
                    error!("events channel closed!");
                    let _ = socket.close().await;
                    break;
                }
            }
        }
    })
}

async fn send_event(socket: &mut WebSocket, event: IdEvent) -> anyhow::Result<()> {
    socket.send(serde_json::to_string(&event)?.into()).await?;

    Ok(())
}