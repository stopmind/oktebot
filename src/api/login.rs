use crate::api::ApiCtx;
use axum::{
    Json,
    extract::State,
    response::{IntoResponse, Response},
};
use log::error;
use reqwest::StatusCode;

pub async fn auth(State(ctx): State<ApiCtx>, body: String) -> Response {
    match ctx.login.check_token(body.as_str()) {
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
