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

//! Host functions provided by Flowtrace to plugins
//!
//! These functions allow plugins to interact with the Flowtrace runtime.

use crate::types::*;

/// Host provides access to Flowtrace runtime functions
pub struct Host;

impl Host {
    /// Log a message to the Flowtrace logs
    pub fn log(level: LogLevel, message: &str) {
        let level_i32 = match level {
            LogLevel::Trace => 0,
            LogLevel::Debug => 1,
            LogLevel::Info => 2,
            LogLevel::Warn => 3,
            LogLevel::Error => 4,
        };
        unsafe {
            host_log(level_i32, message.as_ptr() as i32, message.len() as i32);
        }
    }

    /// Log at trace level
    pub fn trace(message: &str) {
        Self::log(LogLevel::Trace, message);
    }

    /// Log at debug level
    pub fn debug(message: &str) {
        Self::log(LogLevel::Debug, message);
    }

    /// Log at info level
    pub fn info(message: &str) {
        Self::log(LogLevel::Info, message);
    }

    /// Log at warn level
    pub fn warn(message: &str) {
        Self::log(LogLevel::Warn, message);
    }

    /// Log at error level
    pub fn error(message: &str) {
        Self::log(LogLevel::Error, message);
    }

    /// Get plugin configuration as JSON string
    pub fn get_config() -> String {
        unsafe {
            let result = host_get_config();
            read_string_from_result(result)
        }
    }

    /// Get a specific configuration value
    pub fn get_config_value(key: &str) -> Option<String> {
        unsafe {
            let result = host_get_config_value(key.as_ptr() as i32, key.len() as i32);
            if result == 0 {
                None
            } else {
                Some(read_string_from_result(result))
            }
        }
    }

    /// Query traces from the database (requires trace-read capability)
    pub fn query_traces(filter_json: &str, limit: u32) -> Result<Vec<TraceContext>, String> {
        unsafe {
            let result = host_query_traces(
                filter_json.as_ptr() as i32,
                filter_json.len() as i32,
                limit as i32,
            );

            let json = read_string_from_result(result);
            serde_json::from_str(&json).map_err(|e| e.to_string())
        }
    }

    /// Get a single trace by ID (requires trace-read capability)
    pub fn get_trace(trace_id: TraceId) -> Result<Option<TraceContext>, String> {
        let id_str = trace_id.to_uuid();
        unsafe {
            let result = host_get_trace(id_str.as_ptr() as i32, id_str.len() as i32);
            if result == 0 {
                Ok(None)
            } else {
                let json = read_string_from_result(result);
                let trace = serde_json::from_str(&json).map_err(|e| e.to_string())?;
                Ok(Some(trace))
            }
        }
    }

    /// Make an HTTP request (requires network capability)
    pub fn http_request(
        method: &str,
        url: &str,
        headers: &[(String, String)],
        body: Option<&[u8]>,
    ) -> Result<HttpResponse, String> {
        let headers_json = serde_json::to_string(headers).unwrap();
        let body_ptr = body.map(|b| b.as_ptr() as i32).unwrap_or(0);
        let body_len = body.map(|b| b.len() as i32).unwrap_or(0);

        unsafe {
            let result = host_http_request(
                method.as_ptr() as i32,
                method.len() as i32,
                url.as_ptr() as i32,
                url.len() as i32,
                headers_json.as_ptr() as i32,
                headers_json.len() as i32,
                body_ptr,
                body_len,
            );

            let json = read_string_from_result(result);
            serde_json::from_str(&json).map_err(|e| e.to_string())
        }
    }

    /// Generate text embedding (requires embedding capability)
    pub fn embed_text(text: &str) -> Result<Embedding, String> {
        unsafe {
            let result = host_embed_text(text.as_ptr() as i32, text.len() as i32);
            let json = read_string_from_result(result);
            serde_json::from_str(&json).map_err(|e| e.to_string())
        }
    }

    /// Batch embed multiple texts (requires embedding capability)
    pub fn embed_batch(texts: &[String]) -> Result<Vec<Embedding>, String> {
        let texts_json = serde_json::to_string(texts).unwrap();
        unsafe {
            let result = host_embed_batch(texts_json.as_ptr() as i32, texts_json.len() as i32);
            let json = read_string_from_result(result);
            serde_json::from_str(&json).map_err(|e| e.to_string())
        }
    }

    /// Get environment variable (requires env-vars capability)
    pub fn get_env(name: &str) -> Option<String> {
        unsafe {
            let result = host_get_env(name.as_ptr() as i32, name.len() as i32);
            if result == 0 {
                None
            } else {
                Some(read_string_from_result(result))
            }
        }
    }
}

// Helper to read a string from a result (ptr << 32 | len)
unsafe fn read_string_from_result(result: i64) -> String {
    let ptr = (result >> 32) as *const u8;
    let len = (result & 0xFFFFFFFF) as usize;
    let bytes = std::slice::from_raw_parts(ptr, len);
    String::from_utf8_lossy(bytes).into_owned()
}

// External host functions (provided by Flowtrace runtime)
extern "C" {
    fn host_log(level: i32, msg_ptr: i32, msg_len: i32);
    fn host_get_config() -> i64;
    fn host_get_config_value(key_ptr: i32, key_len: i32) -> i64;
    fn host_query_traces(filter_ptr: i32, filter_len: i32, limit: i32) -> i64;
    fn host_get_trace(id_ptr: i32, id_len: i32) -> i64;
    fn host_http_request(
        method_ptr: i32,
        method_len: i32,
        url_ptr: i32,
        url_len: i32,
        headers_ptr: i32,
        headers_len: i32,
        body_ptr: i32,
        body_len: i32,
    ) -> i64;
    fn host_embed_text(text_ptr: i32, text_len: i32) -> i64;
    fn host_embed_batch(texts_ptr: i32, texts_len: i32) -> i64;
    fn host_get_env(name_ptr: i32, name_len: i32) -> i64;
}
