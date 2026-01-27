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

//! Sample Evaluator Plugin for Flowtrace
//!
//! This plugin demonstrates how to create custom evaluators that can be
//! loaded dynamically into Flowtrace.

use serde::{Deserialize, Serialize};

/// Plugin metadata - exported as a C function for dynamic loading
#[no_mangle]
pub extern "C" fn plugin_info() -> *const u8 {
    static INFO: &str = r#"{
        "name": "sample-evaluator",
        "version": "0.1.0",
        "description": "Sample evaluator plugin demonstrating custom evaluators"
    }"#;
    INFO.as_ptr()
}

/// Evaluation result returned by evaluators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    /// Name of the evaluator that produced this result
    pub evaluator: String,
    /// Whether the evaluation passed (for binary evaluators)
    pub passed: Option<bool>,
    /// Score value (for score-based evaluators, 0.0 - 1.0)
    pub score: Option<f64>,
    /// Human-readable explanation of the result
    pub reason: String,
    /// Additional metadata
    pub metadata: serde_json::Value,
}

/// Input for evaluation - typically a trace or span
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalInput {
    /// The content to evaluate
    pub content: String,
    /// Additional context
    pub context: serde_json::Value,
}

/// Sentiment check evaluator
/// 
/// Returns a score from 0.0 (very negative) to 1.0 (very positive)
#[no_mangle]
pub extern "C" fn evaluate_sentiment(input_json: *const u8, input_len: usize) -> *mut u8 {
    let input = unsafe {
        let slice = std::slice::from_raw_parts(input_json, input_len);
        std::str::from_utf8_unchecked(slice)
    };

    let eval_input: EvalInput = match serde_json::from_str(input) {
        Ok(i) => i,
        Err(e) => {
            let error = format!(r#"{{"error": "{}"}}"#, e);
            return string_to_ptr(error);
        }
    };

    // Simple sentiment analysis based on keyword presence
    // In a real implementation, this would use an ML model or API
    let content_lower = eval_input.content.to_lowercase();
    
    let positive_words = ["good", "great", "excellent", "amazing", "wonderful", "helpful", "thank", "perfect", "love", "best"];
    let negative_words = ["bad", "terrible", "awful", "horrible", "wrong", "error", "fail", "hate", "worst", "poor"];
    
    let positive_count: i32 = positive_words.iter()
        .map(|w| content_lower.matches(w).count() as i32)
        .sum();
    
    let negative_count: i32 = negative_words.iter()
        .map(|w| content_lower.matches(w).count() as i32)
        .sum();
    
    let total = (positive_count + negative_count).max(1);
    let score = (positive_count as f64 + total as f64 / 2.0) / (total as f64 + total as f64 / 2.0);
    let score = score.clamp(0.0, 1.0);

    let result = EvalResult {
        evaluator: "sentiment-check".to_string(),
        passed: None,
        score: Some(score),
        reason: format!(
            "Detected {} positive and {} negative indicators",
            positive_count, negative_count
        ),
        metadata: serde_json::json!({
            "positive_count": positive_count,
            "negative_count": negative_count
        }),
    };

    let json = serde_json::to_string(&result).unwrap();
    string_to_ptr(json)
}

/// Length check evaluator
/// 
/// Checks if content length is within specified bounds
#[no_mangle]
pub extern "C" fn evaluate_length(input_json: *const u8, input_len: usize, min_len: usize, max_len: usize) -> *mut u8 {
    let input = unsafe {
        let slice = std::slice::from_raw_parts(input_json, input_len);
        std::str::from_utf8_unchecked(slice)
    };

    let eval_input: EvalInput = match serde_json::from_str(input) {
        Ok(i) => i,
        Err(e) => {
            let error = format!(r#"{{"error": "{}"}}"#, e);
            return string_to_ptr(error);
        }
    };

    let content_len = eval_input.content.len();
    let passed = content_len >= min_len && content_len <= max_len;

    let result = EvalResult {
        evaluator: "length-check".to_string(),
        passed: Some(passed),
        score: None,
        reason: if passed {
            format!("Content length {} is within bounds [{}, {}]", content_len, min_len, max_len)
        } else {
            format!("Content length {} is outside bounds [{}, {}]", content_len, min_len, max_len)
        },
        metadata: serde_json::json!({
            "content_length": content_len,
            "min_length": min_len,
            "max_length": max_len
        }),
    };

    let json = serde_json::to_string(&result).unwrap();
    string_to_ptr(json)
}

/// Toxicity filter evaluator
/// 
/// Returns a toxicity score from 0.0 (not toxic) to 1.0 (very toxic)
#[no_mangle]
pub extern "C" fn evaluate_toxicity(input_json: *const u8, input_len: usize) -> *mut u8 {
    let input = unsafe {
        let slice = std::slice::from_raw_parts(input_json, input_len);
        std::str::from_utf8_unchecked(slice)
    };

    let eval_input: EvalInput = match serde_json::from_str(input) {
        Ok(i) => i,
        Err(e) => {
            let error = format!(r#"{{"error": "{}"}}"#, e);
            return string_to_ptr(error);
        }
    };

    // Simple toxicity detection based on keyword presence
    // In a real implementation, this would use an ML model
    let content_lower = eval_input.content.to_lowercase();
    
    // Very simplified - real implementation would use proper ML
    let toxic_indicators = ["stupid", "idiot", "dumb", "hate", "kill", "die"];
    
    let toxic_count: usize = toxic_indicators.iter()
        .map(|w| content_lower.matches(w).count())
        .sum();
    
    let score = (toxic_count as f64 * 0.2).min(1.0);

    let result = EvalResult {
        evaluator: "toxicity-filter".to_string(),
        passed: None,
        score: Some(score),
        reason: if score < 0.3 {
            "Content appears to be non-toxic".to_string()
        } else if score < 0.7 {
            "Content may contain mildly toxic elements".to_string()
        } else {
            "Content appears to contain toxic elements".to_string()
        },
        metadata: serde_json::json!({
            "toxic_indicator_count": toxic_count
        }),
    };

    let json = serde_json::to_string(&result).unwrap();
    string_to_ptr(json)
}

/// Helper function to convert a String to a raw pointer
/// The caller is responsible for freeing this memory
fn string_to_ptr(s: String) -> *mut u8 {
    let mut bytes = s.into_bytes();
    bytes.push(0); // null terminator
    let ptr = bytes.as_mut_ptr();
    std::mem::forget(bytes);
    ptr
}

/// Free a string that was allocated by this library
#[no_mangle]
pub extern "C" fn free_string(ptr: *mut u8, len: usize) {
    unsafe {
        let _ = Vec::from_raw_parts(ptr, len + 1, len + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sentiment_positive() {
        let input = EvalInput {
            content: "This is a great and wonderful response! Thank you so much!".to_string(),
            context: serde_json::json!({}),
        };
        let json = serde_json::to_string(&input).unwrap();
        let result_ptr = evaluate_sentiment(json.as_ptr(), json.len());
        
        let result_str = unsafe {
            std::ffi::CStr::from_ptr(result_ptr as *const i8)
                .to_string_lossy()
                .into_owned()
        };
        
        let result: EvalResult = serde_json::from_str(&result_str).unwrap();
        assert!(result.score.unwrap() > 0.5);
    }

    #[test]
    fn test_length_check_pass() {
        let input = EvalInput {
            content: "This is a test response with adequate length.".to_string(),
            context: serde_json::json!({}),
        };
        let json = serde_json::to_string(&input).unwrap();
        let result_ptr = evaluate_length(json.as_ptr(), json.len(), 10, 1000);
        
        let result_str = unsafe {
            std::ffi::CStr::from_ptr(result_ptr as *const i8)
                .to_string_lossy()
                .into_owned()
        };
        
        let result: EvalResult = serde_json::from_str(&result_str).unwrap();
        assert!(result.passed.unwrap());
    }

    #[test]
    fn test_toxicity_clean() {
        let input = EvalInput {
            content: "This is a friendly and helpful response.".to_string(),
            context: serde_json::json!({}),
        };
        let json = serde_json::to_string(&input).unwrap();
        let result_ptr = evaluate_toxicity(json.as_ptr(), json.len());
        
        let result_str = unsafe {
            std::ffi::CStr::from_ptr(result_ptr as *const i8)
                .to_string_lossy()
                .into_owned()
        };
        
        let result: EvalResult = serde_json::from_str(&result_str).unwrap();
        assert!(result.score.unwrap() < 0.3);
    }
}
