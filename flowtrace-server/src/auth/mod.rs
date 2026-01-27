// Copyright 2025 Sushanth (https://github.com/sushanthpy)
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use axum::{
    extract::Request as AxumRequest,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use url::form_urlencoded;

pub mod rate_limit;
pub use rate_limit::{extract_client_ip, RateLimitConfig, RateLimitResult, RateLimiter};

// Type alias for the request type we use
type Request = AxumRequest;

/// Authentication context attached to each authenticated request
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub tenant_id: u64,
    pub project_id: Option<u16>,
    pub user_id: Option<String>,
}

/// Authentication error
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Missing authentication credentials")]
    MissingCredentials,

    #[error("Invalid authentication credentials")]
    InvalidCredentials,

    #[error("JWT token validation failed: {0}")]
    JwtValidation(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthError::MissingCredentials => (StatusCode::UNAUTHORIZED, self.to_string()),
            AuthError::InvalidCredentials => (StatusCode::UNAUTHORIZED, self.to_string()),
            AuthError::JwtValidation(_) => (StatusCode::UNAUTHORIZED, self.to_string()),
            AuthError::Unauthorized(_) => (StatusCode::FORBIDDEN, self.to_string()),
        };

        (status, message).into_response()
    }
}

/// JWT claims structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,    // User ID
    pub tenant_id: u64, // Tenant ID
    pub project_id: Option<u16>,
    pub exp: usize, // Expiration time
}

/// Authenticator trait for pluggable auth strategies
pub trait Authenticator: Send + Sync {
    /// Authenticate request by examining headers (synchronous)
    fn authenticate(&self, headers: &axum::http::HeaderMap) -> Result<AuthContext, AuthError>;
}

/// API Key authenticator
pub struct ApiKeyAuth {
    /// Map of API key -> (tenant_id, project_id)
    keys: std::collections::HashMap<String, (u64, Option<u16>)>,
}

impl ApiKeyAuth {
    pub fn new(api_keys: Vec<String>) -> Self {
        let mut keys = std::collections::HashMap::new();

        for key_config in api_keys {
            // Format: "api_key:tenant_id" or "api_key:tenant_id:project_id"
            let parts: Vec<&str> = key_config.split(':').collect();
            if parts.len() >= 2 {
                if let Ok(tenant_id) = parts[1].parse::<u64>() {
                    let project_id = parts.get(2).and_then(|p| p.parse::<u16>().ok());
                    keys.insert(parts[0].to_string(), (tenant_id, project_id));
                }
            }
        }

        Self { keys }
    }
}

impl Authenticator for ApiKeyAuth {
    fn authenticate(&self, headers: &axum::http::HeaderMap) -> Result<AuthContext, AuthError> {
        // Check X-API-Key header
        let api_key = headers
            .get("X-API-Key")
            .or_else(|| headers.get("X-Flowtrace-API-Key"))
            .and_then(|h| h.to_str().ok())
            .ok_or(AuthError::MissingCredentials)?;

        // Validate API key
        let (tenant_id, project_id) = self
            .keys
            .get(api_key)
            .ok_or(AuthError::InvalidCredentials)?;

        Ok(AuthContext {
            tenant_id: *tenant_id,
            project_id: *project_id,
            user_id: None,
        })
    }
}

/// Bearer token (JWT) authenticator
pub struct BearerTokenAuth {
    jwt_secret: Vec<u8>,
}

impl BearerTokenAuth {
    pub fn new(jwt_secret: String) -> Self {
        Self {
            jwt_secret: jwt_secret.into_bytes(),
        }
    }
}

impl Authenticator for BearerTokenAuth {
    fn authenticate(&self, headers: &axum::http::HeaderMap) -> Result<AuthContext, AuthError> {
        // Extract Bearer token from Authorization header
        let auth_header = headers
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .ok_or(AuthError::MissingCredentials)?;

        if !auth_header.starts_with("Bearer ") {
            return Err(AuthError::MissingCredentials);
        }

        let token = &auth_header[7..]; // Remove "Bearer " prefix

        // Validate JWT
        let token_data = jsonwebtoken::decode::<Claims>(
            token,
            &jsonwebtoken::DecodingKey::from_secret(&self.jwt_secret),
            &jsonwebtoken::Validation::default(),
        )
        .map_err(|e| AuthError::JwtValidation(e.to_string()))?;

        Ok(AuthContext {
            tenant_id: token_data.claims.tenant_id,
            project_id: token_data.claims.project_id,
            user_id: Some(token_data.claims.sub),
        })
    }
}

/// Multi-strategy authenticator (tries multiple auth methods)
pub struct MultiAuth {
    strategies: Vec<Arc<dyn Authenticator>>,
}

impl MultiAuth {
    pub fn new(strategies: Vec<Arc<dyn Authenticator>>) -> Self {
        Self { strategies }
    }
}

impl Authenticator for MultiAuth {
    fn authenticate(&self, headers: &axum::http::HeaderMap) -> Result<AuthContext, AuthError> {
        for strategy in &self.strategies {
            if let Ok(ctx) = strategy.authenticate(headers) {
                return Ok(ctx);
            }
        }
        Err(AuthError::InvalidCredentials)
    }
}

/// No-op authenticator for development (no auth required)
pub struct NoAuth {
    default_tenant_id: u64,
}

impl NoAuth {
    pub fn new(default_tenant_id: u64) -> Self {
        Self { default_tenant_id }
    }
}

impl Authenticator for NoAuth {
    fn authenticate(&self, _headers: &axum::http::HeaderMap) -> Result<AuthContext, AuthError> {
        Ok(AuthContext {
            tenant_id: self.default_tenant_id,
            project_id: Some(0),
            user_id: None,
        })
    }
}

/// Authentication middleware
pub async fn auth_middleware(
    auth: axum::Extension<Arc<dyn Authenticator>>,
    mut req: Request,
    next: Next,
) -> Result<Response, AuthError> {
    match auth.authenticate(req.headers()) {
        Ok(ctx) => {
            req.extensions_mut().insert(ctx);
            Ok(next.run(req).await)
        }
        Err(primary_err) => {
            if let Some(api_key) = extract_api_key_from_query(req.uri()) {
                let mut headers = HeaderMap::new();
                if let Ok(value) = HeaderValue::from_str(&api_key) {
                    headers.insert("X-API-Key", value);
                    if let Ok(ctx) = auth.authenticate(&headers) {
                        req.extensions_mut().insert(ctx);
                        return Ok(next.run(req).await);
                    }
                }
            }

            Err(primary_err)
        }
    }
}

/// Authentication middleware with rate limiting
///
/// This middleware should be used for authentication endpoints to prevent brute force attacks
pub async fn auth_with_rate_limit_middleware(
    auth: axum::Extension<Arc<dyn Authenticator>>,
    rate_limiter: axum::Extension<Arc<RateLimiter>>,
    mut req: Request,
    next: Next,
) -> Result<Response, AuthError> {
    // Extract client identifier (IP address or API key)
    let client_id = if let Some(ip) = extract_client_ip(req.headers()) {
        ip
    } else {
        // Fallback to a default identifier if no IP found
        "unknown".to_string()
    };

    // Check rate limit
    match rate_limiter.check_rate_limit(&client_id) {
        RateLimitResult::Allowed { remaining, .. } => {
            // Rate limit OK, proceed with authentication
            match auth.authenticate(req.headers()) {
                Ok(ctx) => {
                    req.extensions_mut().insert(ctx);
                    let mut response = next.run(req).await;

                    // Add rate limit headers
                    let headers = response.headers_mut();
                    headers.insert(
                        "X-RateLimit-Remaining",
                        HeaderValue::from_str(&remaining.to_string())
                            .unwrap_or_else(|_| HeaderValue::from_static("0")),
                    );

                    Ok(response)
                }
                Err(primary_err) => {
                    // Try fallback to query parameter
                    if let Some(api_key) = extract_api_key_from_query(req.uri()) {
                        let mut headers = HeaderMap::new();
                        if let Ok(value) = HeaderValue::from_str(&api_key) {
                            headers.insert("X-API-Key", value);
                            if let Ok(ctx) = auth.authenticate(&headers) {
                                req.extensions_mut().insert(ctx);
                                return Ok(next.run(req).await);
                            }
                        }
                    }

                    Err(primary_err)
                }
            }
        }
        RateLimitResult::RateLimited { retry_after } => {
            // Rate limited - return 429 Too Many Requests
            let mut response = Response::new(
                format!(
                    "Rate limit exceeded. Try again in {} seconds.",
                    retry_after.as_secs()
                )
                .into(),
            );
            *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;

            // Add rate limit headers
            let headers = response.headers_mut();
            headers.insert("X-RateLimit-Remaining", HeaderValue::from_static("0"));
            headers.insert(
                "Retry-After",
                HeaderValue::from_str(&retry_after.as_secs().to_string())
                    .unwrap_or_else(|_| HeaderValue::from_static("60")),
            );

            tracing::warn!(
                "Rate limit exceeded for client: {} (retry after {}s)",
                client_id,
                retry_after.as_secs()
            );

            Ok(response)
        }
    }
}

fn extract_api_key_from_query(uri: &axum::http::Uri) -> Option<String> {
    let query = uri.query()?;
    for (key, value) in form_urlencoded::parse(query.as_bytes()) {
        let key = key.to_ascii_lowercase();
        if key == "api_key" || key == "x-api-key" {
            return Some(value.into_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    #[test]
    fn test_api_key_auth() {
        let auth = ApiKeyAuth::new(vec![
            "test_key:123".to_string(),
            "test_key2:456:1".to_string(),
        ]);

        let mut headers = HeaderMap::new();
        headers.insert("X-API-Key", "test_key".parse().unwrap());

        let ctx = auth.authenticate(&headers).unwrap();
        assert_eq!(ctx.tenant_id, 123);
        assert_eq!(ctx.project_id, None);
    }

    #[test]
    fn test_no_auth() {
        let auth = NoAuth::new(999);
        let headers = HeaderMap::new();

        let ctx = auth.authenticate(&headers).unwrap();
        assert_eq!(ctx.tenant_id, 999);
    }
}
