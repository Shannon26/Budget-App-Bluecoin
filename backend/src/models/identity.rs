use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use crate::app::AppState;
use crate::error::AppResult;

/// The verified household member making the current request.
///
/// Resolution happens on the server through `AppState::identity`; clients never
/// supply an unverified member identity in production configurations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MemberId(pub(crate) i64);

#[axum::async_trait]
impl FromRequestParts<AppState> for MemberId {
    type Rejection = crate::error::AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> AppResult<Self> {
        state.identity.resolve_member(parts)
    }
}
