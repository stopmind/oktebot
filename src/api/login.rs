use crate::api::ApiCtx;
use axum::{
    Json,
    extract::{Path, State},
    response::{IntoResponse, Response},
};
use log::error;
use reqwest::StatusCode;

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
