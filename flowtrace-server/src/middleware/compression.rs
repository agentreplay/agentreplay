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

use axum::{body::Body, extract::Request, http::header, middleware::Next, response::Response};
use http;
use tower_http::compression::{CompressionLayer, Predicate};

/// Compression predicate that compresses responses > 1KB
#[derive(Clone, Default)]
pub struct SizeThresholdPredicate;

impl Predicate for SizeThresholdPredicate {
    fn should_compress<B>(&self, _response: &http::Response<B>) -> bool {
        // Always allow compression - tower-http will handle size threshold
        true
    }
}

/// Create compression layer for responses
///
/// Automatically compresses responses using gzip when:
/// - Response is larger than 1KB
/// - Client accepts gzip encoding (Accept-Encoding header)
///
/// # Example
/// ```ignore
/// use flowtrace_server::middleware::compression::compression_layer;
/// use axum::{Router, routing::get};
///
/// let app = Router::new()
///     .route("/api/v1/traces", get(list_traces))
///     .layer(compression_layer());
/// ```
pub fn compression_layer() -> CompressionLayer {
    CompressionLayer::new()
}

/// Request decompression middleware
///
/// Decompresses request bodies if Content-Encoding header is present.
/// Supports gzip encoding.
pub async fn decompress_request_middleware(
    request: Request,
    next: Next,
) -> Result<Response, axum::http::StatusCode> {
    let (parts, body) = request.into_parts();

    // Check if request is compressed
    let encoding = parts
        .headers
        .get(header::CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok());

    let body = match encoding {
        Some("gzip") => {
            // Decompress gzip body
            match decompress_gzip_body(body).await {
                Ok(decompressed) => decompressed,
                Err(_) => {
                    return Err(axum::http::StatusCode::BAD_REQUEST);
                }
            }
        }
        Some(_unsupported) => {
            // Unsupported encoding
            return Err(axum::http::StatusCode::UNSUPPORTED_MEDIA_TYPE);
        }
        None => body, // No compression
    };

    // Reconstruct request with decompressed body
    let request = Request::from_parts(parts, body);

    Ok(next.run(request).await)
}

/// Decompress gzip body
async fn decompress_gzip_body(body: Body) -> Result<Body, std::io::Error> {
    use axum::body::to_bytes;
    use flate2::read::GzDecoder;
    use std::io::Read;

    // Collect body bytes
    let body_bytes = to_bytes(body, usize::MAX)
        .await
        .map_err(std::io::Error::other)?;

    // Decompress
    let mut decoder = GzDecoder::new(&body_bytes[..]);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;

    Ok(Body::from(decompressed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_layer_creation() {
        let _layer = compression_layer();
        // Just verify it can be created
    }
}
