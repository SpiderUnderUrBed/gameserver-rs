
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::RwLock;
use tokio::time::Instant;
use crate::filesystem::FileResponseMessage;
use crate::AppState;
use crate::ConsoleData;
use crate::Debug;
use crate::InnerData;
use crate::List;
use crate::MessagePayload;

use tokio::sync::Mutex;
use crate::Message;
use futures_util::stream;
use axum::extract::ws::WebSocket;
use crate::WsMessage;
use tokio::time::interval;
use axum::body::Bytes;
use futures_util::SinkExt;

pub struct JsonAssembler {
    pub(crate) buffer: String,
    assembling_for: Option<u64>,
    assembly_deadline: Option<std::time::Instant>,
    assembly_timeout: std::time::Duration,
}

impl JsonAssembler {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            assembling_for: None,
            assembly_deadline: None,
            assembly_timeout: Duration::from_secs(5),
        }
    }

    pub fn check_timeout(&mut self, id: u64) -> Option<std::io::Result<Vec<u8>>> {
        if let Some(deadline) = self.assembly_deadline {
            if Instant::now() > deadline.into() {
                let buffer_content = self.buffer.clone();
                self.buffer.clear();
                self.assembly_deadline = None;
                self.assembling_for = None;

                if buffer_content.is_empty() {
                    return Some(Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        format!("Assembly timeout for request {} with empty buffer", id),
                    )));
                } else {
                    return Some(Ok(buffer_content.into_bytes()));
                }
            }
        }
        None
    }

    pub async fn feed_chunk(&mut self, chunk: &str, id: u64) -> Vec<Vec<u8>> {
        if self.assembly_deadline.is_none() {
            self.assembly_deadline = Some((Instant::now() + self.assembly_timeout).into());
            self.assembling_for = Some(id);
        }

        self.buffer.push_str(chunk);
        let mut completed = Vec::new();

        loop {
            let s = self.buffer.trim_start();
            if s.is_empty() {
                self.buffer.clear();
                break;
            }

            let mut start_pos = None;
            let mut in_string = false;
            let mut escape_next = false;
            let mut brace_count = 0;
            let mut bracket_count = 0;
            let mut found_start = false;

            for (i, c) in s.char_indices() {
                if escape_next {
                    escape_next = false;
                    continue;
                }

                match c {
                    '\\' if in_string => {
                        escape_next = true;
                    }
                    '"' => {
                        in_string = !in_string;
                    }
                    '{' if !in_string => {
                        if !found_start {
                            start_pos = Some(i);
                            found_start = true;
                        }
                        brace_count += 1;
                    }
                    '}' if !in_string => {
                        brace_count -= 1;
                        if brace_count == 0 && found_start {
                            if let Some(start) = start_pos {
                                let candidate = &s[start..=i];

                                match serde_json::from_str::<serde_json::Value>(candidate) {
                                    Ok(_) => {
                                        completed.push(candidate.as_bytes().to_vec());
                                        self.buffer = s[i + 1..].to_string();

                                        self.assembly_deadline = None;
                                        self.assembling_for = None;

                                        return completed;
                                    }
                                    Err(e) => {}
                                }
                            }
                        }
                    }
                    '[' if !in_string => {
                        if !found_start {
                            start_pos = Some(i);
                            found_start = true;
                        }
                        bracket_count += 1;
                    }
                    ']' if !in_string => {
                        bracket_count -= 1;
                        if bracket_count == 0 && found_start && brace_count == 0 {
                            if let Some(start) = start_pos {
                                let candidate = &s[start..=i];

                                match serde_json::from_str::<serde_json::Value>(candidate) {
                                    Ok(_) => {
                                        completed.push(candidate.as_bytes().to_vec());
                                        self.buffer = s[i + 1..].to_string();

                                        self.assembly_deadline = None;
                                        self.assembling_for = None;

                                        return completed;
                                    }
                                    Err(_) => {}
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            if found_start && (brace_count > 0 || bracket_count > 0) {
                break;
            } else if !found_start {
                if let Some(pos) = s.find('{').or_else(|| s.find('[')) {
                    if pos > 0 {
                        self.buffer = s[pos..].to_string();
                        continue;
                    }
                } else {
                    self.buffer.clear();
                    break;
                }
            } else {
                self.buffer.clear();
                break;
            }
        }

        completed
    }
}


pub fn parse_json_objects_in_str<T>(input: &str) -> Vec<Result<T, serde_json::Error>>
where
    T: DeserializeOwned + Debug,
{
    let mut results = Vec::new();
    let mut remaining = input;

    while let Some(start) = remaining.find('{') {
        let mut open_braces = 0usize;
        let mut end_index = None;

        for (i, c) in remaining[start..].chars().enumerate() {
            if c == '{' {
                open_braces += 1;
            } else if c == '}' {
                open_braces -= 1;
                if open_braces == 0 {
                    end_index = Some(start + i + 1);
                    break;
                }
            }
        }

        if let Some(end) = end_index {
            let candidate = &remaining[start..end];
            match serde_json::from_str::<Value>(candidate) {
                Ok(val) => {
                    if let Ok(inner_data) = serde_json::from_value::<InnerData>(val.clone()) {
                        match serde_json::from_str::<T>(&inner_data.data) {
                            Ok(parsed) => results.push(Ok(parsed)),
                            Err(e) => results.push(Err(e)),
                        }
                    } else {
                        match serde_json::from_value::<T>(val) {
                            Ok(parsed) => results.push(Ok(parsed)),
                            Err(e) => results.push(Err(e)),
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


pub async fn value_from_line<T, F>(gameserver_str: &str, filter: F) -> Vec<Result<T, serde_json::Error>>
where
    T: DeserializeOwned + Debug,
    F: Fn(&str) -> bool,
{
    let mut final_values = vec![];

    for line in gameserver_str.lines() {
        let line = line.trim();
        if line.is_empty() || !filter(line) {
            continue;
        }

        match serde_json::from_str::<Value>(line) {
            Ok(line_val) => {
                let mut parsed = false;

                if !parsed {
                    if let Ok(resp_msg) = serde_json::from_value::<FileResponseMessage>(line_val.clone()) {
                        if let Ok(data_str) = String::from_utf8(resp_msg.data.clone()) {
                            if let Ok(result) = serde_json::from_str::<T>(&data_str) {
                                final_values.push(Ok(result));
                            } else {
                                final_values.push(Err(serde_json::from_str::<T>("").unwrap_err()));
                            }
                        }
                        parsed = true;
                    }
                }

                if !parsed {
                    if let Ok(inner_data) = serde_json::from_value::<InnerData>(line_val.clone()) {
                        if let Ok(result) = serde_json::from_str::<T>(&inner_data.data) {
                            final_values.push(Ok(result));
                        } else if let Ok(result) = serde_json::from_value::<T>(serde_json::to_value(&inner_data).unwrap()) {
                            final_values.push(Ok(result));
                        } else {
                            final_values.push(Err(serde_json::from_str::<T>("").unwrap_err()));
                        }
                        parsed = true;
                    }
                }

                if !parsed {
                    if let Ok(console_data) = serde_json::from_value::<ConsoleData>(line_val.clone()) {
                        if let Ok(result) = serde_json::from_str::<T>(&console_data.data) {
                            final_values.push(Ok(result));
                        } else if let Ok(result) = serde_json::from_value::<T>(serde_json::to_value(&console_data).unwrap()) {
                            final_values.push(Ok(result));
                        } else {
                            final_values.push(Err(serde_json::from_str::<T>("").unwrap_err()));
                        }
                        parsed = true;
                    }
                }

                if !parsed {
                    if let Ok(list) = serde_json::from_value::<List>(line_val.clone()) {
                        if let Ok(result) = serde_json::from_value::<T>(serde_json::to_value(&list).unwrap()) {
                            final_values.push(Ok(result));
                        }
                        parsed = true;
                    }
                }

                if !parsed {
                    match serde_json::from_value::<T>(line_val.clone()) {
                        Ok(result) => final_values.push(Ok(result)),
                        Err(e) => {
                            if let Ok(result) = serde_json::from_value::<T>(line_val) {
                                final_values.push(Ok(result));
                            } else {
                                final_values.push(Err(e));
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let partials = parse_json_objects_in_str::<T>(line);
                if partials.is_empty() {
                    if let Ok(val) = serde_json::from_str::<T>(&format!("\"{}\"", line)) {
                        final_values.push(Ok(val));
                    }
                }
                final_values.extend(partials);
            }
        }
    }

    final_values
}


async fn ws_debug(
    conn_id: usize,
    arc_state: Arc<RwLock<AppState>>,
    sender: Arc<Mutex<stream::SplitSink<WebSocket, WsMessage>>>,
    receiver: &mut stream::SplitStream<WebSocket>,
) {
    let state = arc_state.write().await;
    // Ping task with more visible logging
    let ping_task = {
        let conn_id = conn_id;
        let sender = Arc::clone(&sender);
        let mut interval = interval(Duration::from_secs(30));

        tokio::spawn(async move {
            println!("[Conn {}] PING TASK STARTED", conn_id);

            loop {
                interval.tick().await;
                println!("[Conn {}] SENDING PING", conn_id); // <-- Log ping attempts

                let mut sender = sender.lock().await;
                match sender.send(Message::Ping(Bytes::new())).await {
                    Ok(_) => println!("[Conn {}] PING SENT SUCCESSFULLY", conn_id),
                    Err(e) => {
                        println!("[Conn {}] PING FAILED: {}", conn_id, e);
                        break;
                    }
                }
            }

            println!("[Conn {}] PING TASK EXITING", conn_id);
        })
    };

    // Broadcast receiver task with more visible logging
    let broadcast_task = {
        let conn_id = conn_id;
        let sender = Arc::clone(&sender);
        let mut broadcast_rx = state.ws_tx.subscribe();

        tokio::spawn(async move {
            println!("[Conn {}] BROADCAST TASK STARTED", conn_id);

            while let Ok(msg) = broadcast_rx.recv().await {
                println!("[Conn {}] RECEIVED BROADCAST: {}", conn_id, msg);

                let mut sender = sender.lock().await;
                match sender.send(Message::Text(msg.into())).await {
                    Ok(_) => println!("[Conn {}] FORWARDED MESSAGE", conn_id),
                    Err(e) => {
                        println!("[Conn {}] FAILED TO FORWARD: {}", conn_id, e);
                        break;
                    }
                }
            }

            println!("[Conn {}] BROADCAST TASK EXITING", conn_id);
        })
    };

    // Main message processing loop with more visible logging
    println!("[Conn {}] STARTING MESSAGE PROCESSING", conn_id);

    while let Some(result) = receiver.next().await {
        match result {
            Ok(Message::Text(text)) => {
                println!("[Conn {}] RECEIVED TEXT: {}", conn_id, text);

                match serde_json::from_str::<MessagePayload>(&text) {
                    Ok(payload) => {
                        println!("[Conn {}] PARSED PAYLOAD: {:?}", conn_id, payload);

                        match serde_json::to_vec(&payload) {
                            Ok(mut bytes) => {
                                bytes.push(b'\n');
                                println!("[Conn {}] SERIALIZED TO {} BYTES", conn_id, bytes.len());

                                match state.tcp_tx.send(bytes) {
                                    Ok(_) => println!("[Conn {}] SENT TO TCP", conn_id),
                                    Err(e) => println!("[Conn {}] TCP SEND FAILED: {}", conn_id, e),
                                }
                            }
                            Err(e) => println!("[Conn {}] SERIALIZATION FAILED: {}", conn_id, e),
                        }
                    }
                    Err(e) => println!("[Conn {}] PARSE FAILED: {}", conn_id, e),
                }
            }
            Ok(Message::Binary(bin)) => {
                println!("[Conn {}] RECEIVED BINARY ({} bytes)", conn_id, bin.len());
            }
            Ok(Message::Ping(data)) => {
                println!("[Conn {}] RECEIVED PING ({} bytes)", conn_id, data.len());
            }
            Ok(Message::Pong(data)) => {
                println!("[Conn {}] RECEIVED PONG ({} bytes)", conn_id, data.len());
            }
            Ok(Message::Close(frame)) => {
                println!("[Conn {}] CLOSE FRAME: {:?}", conn_id, frame);
                break;
            }
            Err(e) => {
                println!("[Conn {}] WEBSOCKET ERROR: {}", conn_id, e);
                break;
            }
        }
    }

    println!("[Conn {}] SHUTTING DOWN", conn_id);
    ping_task.abort();
    broadcast_task.abort();

    match ping_task.await {
        Ok(_) => println!("[Conn {}] PING TASK SHUT DOWN", conn_id),
        Err(e) => println!("[Conn {}] PING TASK ERROR: {:?}", conn_id, e),
    }

    match broadcast_task.await {
        Ok(_) => println!("[Conn {}] BROADCAST TASK SHUT DOWN", conn_id),
        Err(e) => println!("[Conn {}] BROADCAST TASK ERROR: {:?}", conn_id, e),
    }
}