//! Parse agent JSONL events into a typed shape both sinks can render.
//!
//! Claude and Codex use different JSONL event shapes. This module
//! normalizes both into [`ParsedAgentEvent`] so the console and TUI do
//! not care which provider produced the stream.

use std::path::Path;

use crate::flow::AgentProvider;

#[derive(Debug, Clone)]
pub enum ParsedAgentEvent {
    Init {
        model: String,
    },
    AssistantText {
        text: String,
    },
    ToolUse {
        name: String,
        target: ToolUseTarget,
    },
    ToolResult {
        first_line: String,
        is_error: bool,
    },
    Done {
        subtype: String,
        num_turns: u64,
        total_cost_usd: f64,
    },
    /// Event we don't render specially (rare; system messages other
    /// than `init`, etc.).
    Other,
}

#[derive(Debug, Clone)]
pub enum ToolUseTarget {
    /// `file_path` argument, e.g. Read / Edit / Write.
    File(String),
    /// `command` argument, e.g. Bash.
    Command(String),
    /// `pattern` argument, e.g. Grep.
    Pattern(String),
    /// Any other tool whose first text-ish argument we picked.
    Description(String),
    /// No useful one-line summary available.
    None,
}

/// Parse one JSONL event value into [`ParsedAgentEvent`].
/// Returns a `Vec` because `assistant` events can carry both text
/// content and tool_use blocks in the same message.
pub fn parse(provider: AgentProvider, ev: &serde_json::Value) -> Vec<ParsedAgentEvent> {
    match provider {
        AgentProvider::Claude => parse_claude(ev),
        AgentProvider::Codex => parse_codex(ev),
    }
}

fn parse_claude(ev: &serde_json::Value) -> Vec<ParsedAgentEvent> {
    let kind = match ev.get("type").and_then(|v| v.as_str()) {
        Some(k) => k,
        None => return vec![],
    };
    match kind {
        "system" => {
            if ev.get("subtype").and_then(|v| v.as_str()) == Some("init") {
                let model = ev
                    .get("model")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                vec![ParsedAgentEvent::Init { model }]
            } else {
                vec![ParsedAgentEvent::Other]
            }
        }
        "assistant" => parse_assistant(ev),
        "user" => parse_user(ev),
        "result" => {
            let subtype = ev
                .get("subtype")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let num_turns = ev.get("num_turns").and_then(|v| v.as_u64()).unwrap_or(0);
            let total_cost_usd = ev
                .get("total_cost_usd")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            vec![ParsedAgentEvent::Done {
                subtype,
                num_turns,
                total_cost_usd,
            }]
        }
        _ => vec![ParsedAgentEvent::Other],
    }
}

fn parse_codex(ev: &serde_json::Value) -> Vec<ParsedAgentEvent> {
    let kind = ev
        .get("type")
        .or_else(|| ev.get("event"))
        .or_else(|| ev.get("msg_type"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let lower = kind.to_ascii_lowercase();

    if lower == "thread.started" {
        let model = string_at(ev, &["model", "model_slug"])
            .or_else(|| string_at_path(ev, &["payload", "model"]))
            .unwrap_or_else(|| "unknown".to_string());
        return vec![ParsedAgentEvent::Init { model }];
    }

    if matches!(lower.as_str(), "turn.started" | "turn.failed" | "turn.aborted") {
        return vec![ParsedAgentEvent::Other];
    }

    if lower == "turn.completed" || lower == "result" {
        return vec![ParsedAgentEvent::Done {
            subtype: string_at(ev, &["subtype", "status"]).unwrap_or_else(|| "success".into()),
            num_turns: ev.get("num_turns").and_then(|v| v.as_u64()).unwrap_or(0),
            total_cost_usd: ev
                .get("total_cost_usd")
                .or_else(|| ev.get("cost_usd"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
        }];
    }

    if matches!(lower.as_str(), "item.started" | "item.completed") {
        return parse_codex_item(ev, lower == "item.started");
    }

    if lower.contains("session") || lower.contains("init") || lower == "started" {
        let model = string_at(ev, &["model", "model_slug"])
            .or_else(|| string_at_path(ev, &["payload", "model"]))
            .unwrap_or_else(|| "unknown".to_string());
        return vec![ParsedAgentEvent::Init { model }];
    }

    if lower.contains("tool") || lower.contains("exec") || lower.contains("command") {
        if lower.contains("result") || lower.contains("output") || lower.contains("end") {
            return vec![ParsedAgentEvent::ToolResult {
                first_line: first_text(ev).unwrap_or_default(),
                is_error: bool_at(ev, &["is_error", "error"])
                    || string_at(ev, &["status"]).as_deref() == Some("failed"),
            }];
        }
        let name = string_at(ev, &["name", "tool", "call_id"])
            .or_else(|| string_at_path(ev, &["payload", "name"]))
            .unwrap_or_else(|| "tool".to_string());
        let target = ev
            .get("input")
            .or_else(|| ev.get("arguments"))
            .or_else(|| ev.get("payload"))
            .map(tool_target)
            .unwrap_or_else(|| {
                string_at(ev, &["command", "cmd"])
                    .map(ToolUseTarget::Command)
                    .unwrap_or(ToolUseTarget::None)
            });
        return vec![ParsedAgentEvent::ToolUse { name, target }];
    }

    if lower.contains("assistant") || lower.contains("message") || lower.contains("text") {
        if let Some(text) = first_text(ev) {
            return vec![ParsedAgentEvent::AssistantText { text }];
        }
    }

    if lower.contains("done") || lower.contains("result") {
        return vec![ParsedAgentEvent::Done {
            subtype: string_at(ev, &["subtype", "status"]).unwrap_or_else(|| "success".into()),
            num_turns: ev.get("num_turns").and_then(|v| v.as_u64()).unwrap_or(0),
            total_cost_usd: ev
                .get("total_cost_usd")
                .or_else(|| ev.get("cost_usd"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
        }];
    }

    if let Some(text) = first_text(ev) {
        if !text.trim().is_empty() {
            return vec![ParsedAgentEvent::AssistantText { text }];
        }
    }
    vec![ParsedAgentEvent::Other]
}

fn parse_codex_item(ev: &serde_json::Value, started: bool) -> Vec<ParsedAgentEvent> {
    let item = match ev.get("item") {
        Some(item) => item,
        None => return vec![ParsedAgentEvent::Other],
    };
    let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match item_type {
        "agent_message" if !started => item
            .get("text")
            .and_then(|v| v.as_str())
            .filter(|text| !text.trim().is_empty())
            .map(|text| {
                vec![ParsedAgentEvent::AssistantText {
                    text: text.to_string(),
                }]
            })
            .unwrap_or_else(|| vec![ParsedAgentEvent::Other]),
        "command_execution" if started => {
            let command = item
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            vec![ParsedAgentEvent::ToolUse {
                name: "command".to_string(),
                target: if command.is_empty() {
                    ToolUseTarget::None
                } else {
                    ToolUseTarget::Command(command)
                },
            }]
        }
        "command_execution" if !started => vec![ParsedAgentEvent::ToolResult {
            first_line: item
                .get("aggregated_output")
                .and_then(|v| v.as_str())
                .and_then(|s| s.lines().find(|line| !line.trim().is_empty()))
                .unwrap_or_else(|| item.get("status").and_then(|v| v.as_str()).unwrap_or(""))
                .to_string(),
            is_error: item.get("exit_code").and_then(|v| v.as_i64()).unwrap_or(0) != 0
                || item.get("status").and_then(|v| v.as_str()) == Some("failed"),
        }],
        "file_change" if started => {
            let path = first_codex_file_change_path(item);
            vec![ParsedAgentEvent::ToolUse {
                name: "file_change".to_string(),
                target: path.map(ToolUseTarget::File).unwrap_or(ToolUseTarget::None),
            }]
        }
        "file_change" if !started => vec![ParsedAgentEvent::ToolResult {
            first_line: first_codex_file_change_path(item).unwrap_or_else(|| "file change".into()),
            is_error: item.get("status").and_then(|v| v.as_str()) == Some("failed"),
        }],
        _ => vec![ParsedAgentEvent::Other],
    }
}

fn first_codex_file_change_path(item: &serde_json::Value) -> Option<String> {
    item.get("changes")
        .and_then(|v| v.as_array())
        .and_then(|changes| changes.first())
        .and_then(|change| change.get("path"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

fn parse_assistant(ev: &serde_json::Value) -> Vec<ParsedAgentEvent> {
    let content = match ev
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
    {
        Some(c) => c,
        None => return vec![],
    };
    let mut out = Vec::new();
    for c in content {
        match c.get("type").and_then(|v| v.as_str()) {
            Some("text") => {
                if let Some(t) = c.get("text").and_then(|v| v.as_str()) {
                    out.push(ParsedAgentEvent::AssistantText {
                        text: t.to_string(),
                    });
                }
            }
            Some("tool_use") => {
                let name = c
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
                    .to_string();
                let target = c
                    .get("input")
                    .map(tool_target)
                    .unwrap_or(ToolUseTarget::None);
                out.push(ParsedAgentEvent::ToolUse { name, target });
            }
            _ => {}
        }
    }
    out
}

fn parse_user(ev: &serde_json::Value) -> Vec<ParsedAgentEvent> {
    let content = match ev
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
    {
        Some(c) => c,
        None => return vec![],
    };
    let mut out = Vec::new();
    for c in content {
        if c.get("type").and_then(|v| v.as_str()) != Some("tool_result") {
            continue;
        }
        let is_error = c.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false);
        let first_line = tool_result_first_line(c);
        out.push(ParsedAgentEvent::ToolResult {
            first_line,
            is_error,
        });
    }
    out
}

fn string_at(ev: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|k| ev.get(*k).and_then(|v| v.as_str()).map(str::to_string))
}

fn string_at_path(ev: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut cur = ev;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(str::to_string)
}

fn bool_at(ev: &serde_json::Value, keys: &[&str]) -> bool {
    keys.iter()
        .any(|k| ev.get(*k).and_then(|v| v.as_bool()).unwrap_or(false))
}

fn first_text(ev: &serde_json::Value) -> Option<String> {
    for key in ["text", "message", "content", "output", "delta"] {
        if let Some(v) = ev.get(key) {
            if let Some(s) = v.as_str() {
                return Some(s.lines().next().unwrap_or("").to_string());
            }
            if let Some(arr) = v.as_array() {
                for item in arr {
                    if let Some(s) = item.as_str() {
                        return Some(s.lines().next().unwrap_or("").to_string());
                    }
                    if let Some(s) = item.get("text").and_then(|v| v.as_str()) {
                        return Some(s.lines().next().unwrap_or("").to_string());
                    }
                }
            }
        }
    }
    None
}

fn tool_target(input: &serde_json::Value) -> ToolUseTarget {
    if let Some(p) = input.get("file_path").and_then(|v| v.as_str()) {
        return ToolUseTarget::File(p.to_string());
    }
    if let Some(c) = input.get("command").and_then(|v| v.as_str()) {
        let first = c.lines().next().unwrap_or("");
        return ToolUseTarget::Command(first.to_string());
    }
    if let Some(p) = input.get("pattern").and_then(|v| v.as_str()) {
        return ToolUseTarget::Pattern(p.to_string());
    }
    if let Some(d) = input.get("description").and_then(|v| v.as_str()) {
        return ToolUseTarget::Description(d.to_string());
    }
    ToolUseTarget::None
}

fn tool_result_first_line(item: &serde_json::Value) -> String {
    // `content` is a string OR an array of content blocks. Take the
    // first text-ish chunk.
    let v = match item.get("content") {
        Some(v) => v,
        None => return String::new(),
    };
    if let Some(s) = v.as_str() {
        return s.lines().next().unwrap_or("").to_string();
    }
    if let Some(arr) = v.as_array() {
        for c in arr {
            if let Some(t) = c.get("text").and_then(|v| v.as_str()) {
                return t.lines().next().unwrap_or("").to_string();
            }
        }
    }
    String::new()
}

/// Strip a path prefix from `p` if `p` lives inside `root`. Returns
/// the path relative to root, or `p` unchanged.
pub fn relativize(p: &str, root: &Path) -> String {
    let root_str = root.to_string_lossy();
    p.strip_prefix(root_str.as_ref())
        .map(|s| s.trim_start_matches('/').to_string())
        .unwrap_or_else(|| p.to_string())
}

pub fn trim_to(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_init() {
        let v: serde_json::Value =
            serde_json::from_str(r#"{"type":"system","subtype":"init","model":"claude-opus-4-7"}"#)
                .unwrap();
        let out = parse(AgentProvider::Claude, &v);
        assert!(matches!(
            out.as_slice(),
            [ParsedAgentEvent::Init { model }] if model == "claude-opus-4-7"
        ));
    }

    #[test]
    fn parses_assistant_with_text_and_tool_use() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{
                "type":"assistant",
                "message":{"content":[
                    {"type":"text","text":"reading…"},
                    {"type":"tool_use","name":"Edit","input":{"file_path":"/repo/g.pest"}}
                ]}
            }"#,
        )
        .unwrap();
        let out = parse(AgentProvider::Claude, &v);
        assert_eq!(out.len(), 2);
        assert!(matches!(&out[0], ParsedAgentEvent::AssistantText { text } if text == "reading…"));
        assert!(matches!(
            &out[1],
            ParsedAgentEvent::ToolUse { name, target: ToolUseTarget::File(p) }
                if name == "Edit" && p == "/repo/g.pest"
        ));
    }

    #[test]
    fn parses_tool_result_first_line_only() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{
                "type":"user",
                "message":{"content":[
                    {"type":"tool_result","content":"first line\nsecond line"}
                ]}
            }"#,
        )
        .unwrap();
        let out = parse(AgentProvider::Claude, &v);
        assert!(matches!(
            &out[0],
            ParsedAgentEvent::ToolResult { first_line, is_error: false }
                if first_line == "first line"
        ));
    }

    #[test]
    fn parses_done() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{"type":"result","subtype":"success","num_turns":12,"total_cost_usd":0.0234}"#,
        )
        .unwrap();
        let out = parse(AgentProvider::Claude, &v);
        assert!(matches!(
            &out[0],
            ParsedAgentEvent::Done { subtype, num_turns: 12, total_cost_usd }
                if subtype == "success" && (total_cost_usd - 0.0234).abs() < 1e-6
        ));
    }

    #[test]
    fn parses_codex_message_text() {
        let v: serde_json::Value =
            serde_json::from_str(r#"{"type":"assistant_message","message":"hello"}"#).unwrap();
        let out = parse(AgentProvider::Codex, &v);
        assert!(matches!(&out[0], ParsedAgentEvent::AssistantText { text } if text == "hello"));
    }

    #[test]
    fn parses_codex_item_completed_agent_message_as_text() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{"type":"item.completed","item":{"type":"agent_message","text":"done editing"}}"#,
        )
        .unwrap();
        let out = parse(AgentProvider::Codex, &v);
        assert!(matches!(&out[0], ParsedAgentEvent::AssistantText { text } if text == "done editing"));
    }

    #[test]
    fn parses_codex_item_completed_command_as_tool_result() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{
                "type":"item.completed",
                "item":{
                    "type":"command_execution",
                    "aggregated_output":"first line\nsecond line",
                    "exit_code":0,
                    "status":"completed"
                }
            }"#,
        )
        .unwrap();
        let out = parse(AgentProvider::Codex, &v);
        assert!(matches!(
            &out[0],
            ParsedAgentEvent::ToolResult { first_line, is_error: false }
                if first_line == "first line"
        ));
    }

    #[test]
    fn parses_codex_turn_completed_as_done() {
        let v: serde_json::Value =
            serde_json::from_str(r#"{"type":"turn.completed","status":"success"}"#).unwrap();
        let out = parse(AgentProvider::Codex, &v);
        assert!(matches!(&out[0], ParsedAgentEvent::Done { subtype, .. } if subtype == "success"));
    }
}
