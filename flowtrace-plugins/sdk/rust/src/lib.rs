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

//! Flowtrace Plugin SDK for Rust
//!
//! This SDK provides types and macros for building Flowtrace plugins in Rust.
//! Plugins are compiled to WASM and loaded by the Flowtrace runtime.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use flowtrace_plugin_sdk::prelude::*;
//!
//! struct MyEvaluator;
//!
//! impl Evaluator for MyEvaluator {
//!     fn evaluate(&self, trace: TraceContext) -> Result<EvalResult, String> {
//!         let score = calculate_score(&trace);
//!         Ok(EvalResult {
//!             evaluator_id: "my-evaluator".into(),
//!             passed: score > 0.7,
//!             confidence: score,
//!             explanation: Some(format!("Score: {:.2}", score)),
//!             ..Default::default()
//!         })
//!     }
//!     
//!     fn get_metadata(&self) -> PluginMetadata {
//!         PluginMetadata {
//!             id: "my-evaluator".into(),
//!             name: "My Custom Evaluator".into(),
//!             version: "1.0.0".into(),
//!             description: "Evaluates traces using custom logic".into(),
//!             ..Default::default()
//!         }
//!     }
//! }
//!
//! export_evaluator!(MyEvaluator);
//! ```
//!
//! # Building
//!
//! ```bash
//! cargo build --target wasm32-wasip1 --release
//! ```

pub mod embedding;
pub mod evaluator;
pub mod exporter;
pub mod host;
pub mod types;

/// Prelude for convenient imports
pub mod prelude {
    pub use crate::embedding::EmbeddingProvider;
    pub use crate::evaluator::Evaluator;
    pub use crate::exporter::Exporter;
    pub use crate::host::Host;
    pub use crate::types::*;
}

pub use evaluator::Evaluator;
pub use host::Host;
pub use types::*;

/// Export an evaluator plugin
#[macro_export]
macro_rules! export_evaluator {
    ($evaluator:ty) => {
        static EVALUATOR: std::sync::OnceLock<$evaluator> = std::sync::OnceLock::new();

        fn get_evaluator() -> &'static $evaluator {
            EVALUATOR.get_or_init(|| <$evaluator>::default())
        }

        #[no_mangle]
        pub extern "C" fn evaluate(trace_ptr: i32, trace_len: i32) -> i64 {
            let trace_bytes =
                unsafe { std::slice::from_raw_parts(trace_ptr as *const u8, trace_len as usize) };

            let trace: $crate::TraceContext = match serde_json::from_slice(trace_bytes) {
                Ok(t) => t,
                Err(e) => {
                    let error = format!("{{\"error\": \"{}\"}}", e);
                    return $crate::return_string(error);
                }
            };

            match get_evaluator().evaluate(trace) {
                Ok(result) => {
                    let json = serde_json::to_string(&result).unwrap();
                    $crate::return_string(json)
                }
                Err(e) => {
                    let error = format!("{{\"error\": \"{}\"}}", e);
                    $crate::return_string(error)
                }
            }
        }

        #[no_mangle]
        pub extern "C" fn get_metadata() -> i64 {
            let metadata = get_evaluator().get_metadata();
            let json = serde_json::to_string(&metadata).unwrap();
            $crate::return_string(json)
        }
    };
}

/// Export an embedding provider plugin
#[macro_export]
macro_rules! export_embedding_provider {
    ($provider:ty) => {
        static PROVIDER: std::sync::OnceLock<$provider> = std::sync::OnceLock::new();

        fn get_provider() -> &'static $provider {
            PROVIDER.get_or_init(|| <$provider>::default())
        }

        #[no_mangle]
        pub extern "C" fn embed(text_ptr: i32, text_len: i32) -> i64 {
            let text = unsafe {
                std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                    text_ptr as *const u8,
                    text_len as usize,
                ))
            };

            match get_provider().embed(text) {
                Ok(embedding) => {
                    let json = serde_json::to_string(&embedding).unwrap();
                    $crate::return_string(json)
                }
                Err(e) => {
                    let error = format!("{{\"error\": \"{}\"}}", e);
                    $crate::return_string(error)
                }
            }
        }

        #[no_mangle]
        pub extern "C" fn dimension() -> u32 {
            get_provider().dimension()
        }

        #[no_mangle]
        pub extern "C" fn max_tokens() -> u32 {
            get_provider().max_tokens()
        }
    };
}

/// Helper to return a string from WASM (returns ptr << 32 | len)
#[doc(hidden)]
pub fn return_string(s: String) -> i64 {
    let bytes = s.into_bytes();
    let len = bytes.len() as i64;
    let ptr = bytes.as_ptr() as i64;
    std::mem::forget(bytes);
    (ptr << 32) | len
}

/// Allocate memory for host to write into
#[no_mangle]
pub extern "C" fn alloc(size: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(size);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Free memory allocated by alloc
///
/// # Safety
/// The pointer must have been allocated by `alloc` with the same size.
#[no_mangle]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, size: usize) {
    let _ = Vec::from_raw_parts(ptr, 0, size);
}
