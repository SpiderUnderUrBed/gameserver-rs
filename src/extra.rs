use std::time::Duration;

use crate::ConsoleData;
use crate::Debug;
use crate::InnerData;
use futures_util::StreamExt;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::time::Instant;


pub fn parse_json_objects_in_str<T>(input: &str) -> Vec<Result<T, serde_json::Error>>
where
    T: DeserializeOwned + Debug,
{
    let mut results = Vec::new();
    let mut remaining = input;

    while let Some(start) = remaining.find('{') {
        let bytes = remaining[start..].as_bytes();
        let mut open_braces = 0usize;
        let mut in_string = false;
        let mut escaped = false;
        let mut end_index = None;

        for (i, &b) in bytes.iter().enumerate() {
            if escaped {
                escaped = false;
                continue;
            }
            match b {
                b'\\' if in_string => escaped = true,
                b'"' => in_string = !in_string,
                b'{' if !in_string => open_braces += 1,
                b'}' if !in_string => {
                    open_braces -= 1;
                    if open_braces == 0 {
                        end_index = Some(start + i + 1);
                        break;
                    }
                }
                _ => {}
            }
        }

        if let Some(end) = end_index {
            let candidate = &remaining[start..end];
            match serde_json::from_str::<Value>(candidate) {
                Ok(val) => {
                    if let Ok(console) = serde_json::from_value::<ConsoleData>(val.clone()) {
                        if let Ok(inner) = serde_json::from_str::<InnerData>(&console.data) {
                            // Reconstruct a ConsoleData with inner.data as the payload
                            let reconstructed = ConsoleData {
                                authcode: console.authcode.clone(),
                                data: inner.data,
                                r#type: console.r#type.clone(),
                            };
                            if let Ok(parsed) = serde_json::from_value::<T>(
                                serde_json::to_value(reconstructed).unwrap(),
                            ) {
                                results.push(Ok(parsed));
                            }
                        } else if let Ok(parsed) = serde_json::from_str::<T>(&console.data) {
                            results.push(Ok(parsed));
                        } else {
                        }
                    } else {
                        match serde_json::from_value::<T>(val) {
                            Ok(parsed) => results.push(Ok(parsed)),
                            Err(e) => {
                                results.push(Err(e));
                            }
                        }
                    }
                }
                Err(e) => {
                    results.push(Err(e));
                }
            }
            remaining = &remaining[end..];
        } else {
            break;
        }
    }

    results
}
pub async fn value_from_line<T, F>(
    gameserver_str: &str,
    filter: F,
) -> Vec<Result<T, serde_json::Error>>
where
    T: DeserializeOwned + Debug,
    F: Fn(&str) -> bool,
{
    let mut final_values = vec![];
    for line in gameserver_str.lines() {
        if line.is_empty() || !filter(line) {
            continue;
        }
        let partials = parse_json_objects_in_str::<T>(line);
        if partials.is_empty() {
            if let Ok(val) = serde_json::from_str::<T>(&format!("\"{}\"", line)) {
                final_values.push(Ok(val));
            }
        }
        final_values.extend(partials);
    }
    final_values
}
