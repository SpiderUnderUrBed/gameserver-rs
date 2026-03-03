use std::io;
use std::time::{Duration, Instant};

use serde_json;

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
            let instant_deadline: Instant = deadline.into();
            if Instant::now() > instant_deadline {
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
