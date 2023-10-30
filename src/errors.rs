use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

pub(crate) struct AppError(pub(crate) anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let message = self.0.to_string();
        let message = json!({"error": message});

        (StatusCode::BAD_REQUEST, Json(message)).into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
