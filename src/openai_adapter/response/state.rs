//! DeepSeek Patch 状态机 —— 解析 p/o/v 路径操作并产出增量帧
//!
//! 本模块对齐 chat.deepseek.com 前端 SSR delta 解析算法（DeltaParser + rm 类）：
//! - `p` / `o` 跨事件持久化（后续事件可省略）
//! - `o` 默认值为 "SET"
//! - BATCH 递归分解，子项路径前置父路径（使用独立子解析器）
//! - APPEND 对字符串 = `+=`，不存在 snapshot 替换语义

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;
use pin_project_lite::pin_project;

use log::{trace, warn};

use crate::openai_adapter::OpenAIAdapterError;

use super::sse_parser::SseEvent;

const FRAG_THINK: &str = "THINK";
const FRAG_RESPONSE: &str = "RESPONSE";

/// 从 DeepSeek 流中解析出的单帧增量
#[derive(Debug, Clone)]
pub enum DsFrame {
    /// event: ready，用于生成 delta.role = assistant
    Role,
    /// THINK fragment 追加的文本
    ThinkDelta(String),
    /// RESPONSE fragment 追加的文本
    ContentDelta(String),
    /// response/status 变化
    Status(String),
    /// accumulated_token_usage 数值
    Usage(u32),
}

#[derive(Debug, Default)]
struct Fragment {
    ty: String,
    content: String,
}

/// 维护 DeepSeek 响应的 patch 状态，产出可供 converter 消费的增量帧
///
/// current_path / current_op 跨事件持久化，默认值对齐前端 DeltaParser：
/// - `current_op` 默认 "SET"（初始快照、status 更新等不显式带 `o` 时）
/// - `current_path` 默认 None（初始快照无 `p` 时进入特殊处理）
#[derive(Debug, Default)]
pub struct DsState {
    current_path: Option<String>,
    current_op: Option<String>,
    fragments: Vec<Fragment>,
    status: Option<String>,
    accumulated_token_usage: Option<u32>,
}

impl DsState {
    /// 消费一个 SSE 事件，返回零个或多个增量帧
    pub fn apply_event(&mut self, evt: &SseEvent) -> Vec<DsFrame> {
        let mut frames = Vec::new();

        if let Some("ready") = evt.event.as_deref() {
            frames.push(DsFrame::Role);
        }

        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&evt.data) {
            frames.extend(self.apply_patch_value(val));
        }

        frames
    }

    /// 应用单条 `p/o/v` 事件（含跨事件持久化 + BATCH 分解 + 初始快照处理）
    fn apply_patch_value(&mut self, val: serde_json::Value) -> Vec<DsFrame> {
        // 1. p/o 跨事件持久化
        if let Some(p) = val.get("p").and_then(|v| v.as_str()) {
            self.current_path = Some(p.to_string());
        }
        if let Some(o) = val.get("o").and_then(|v| v.as_str()) {
            self.current_op = Some(o.to_string());
        }

        let op = self.current_op.as_deref().unwrap_or("SET").to_string();
        let path = self.current_path.as_deref().unwrap_or("").to_string();

        let Some(v) = val.get("v") else {
            return Vec::new();
        };

        // 2. 初始快照：无 path 且 v 含 response（前端全量状态初始化）
        if self.current_path.is_none()
            && let Some(response) = v.get("response")
        {
            return self.apply_initial_snapshot(response);
        }

        // 3. BATCH 分解：使用独立子解析器，不污染外层 path/op
        if op == "BATCH" {
            if v.is_array() {
                return self.apply_batch(&path, v);
            }
            // 非数组 v 带 BATCH：op 是上一个事件遗留的，此事件实际上是一个 SET。
            // （实际流中 status/usage 事件会显式带 o="SET"，此处做防御处理。）
            return self.apply_path(&path, "SET", v);
        }

        // 4. 单条 SET / APPEND 操作
        self.apply_path(&path, &op, v)
    }

    /// BATCH 递归分解。使用本地子解析器状态（sub_path / sub_op），
    /// 不修改 self.current_path / self.current_op，保持外层状态不变。
    fn apply_batch(&mut self, parent_path: &str, v: &serde_json::Value) -> Vec<DsFrame> {
        let mut frames = Vec::new();
        let Some(arr) = v.as_array() else {
            return frames;
        };

        // 子解析器独立状态（对齐前端 DeltaParser：BATCH 内建新解析器）
        let (mut sub_path, mut sub_op) = (String::new(), "SET".to_string());

        for item in arr {
            if let Some(p) = item.get("p").and_then(|v| v.as_str()) {
                sub_path = p.to_string();
            }
            if let Some(o) = item.get("o").and_then(|v| v.as_str()) {
                sub_op = o.to_string();
            }

            let Some(v) = item.get("v") else {
                continue;
            };

            if sub_op == "BATCH" {
                // 嵌套 BATCH：先拼接完整路径再递归
                let nested = if parent_path.is_empty() {
                    sub_path.clone()
                } else if sub_path.is_empty() {
                    parent_path.to_string()
                } else {
                    format!("{}/{}", parent_path, sub_path)
                };
                frames.extend(self.apply_batch(&nested, v));
            } else {
                let full_path = if parent_path.is_empty() {
                    sub_path.clone()
                } else if sub_path.is_empty() {
                    parent_path.to_string()
                } else {
                    format!("{}/{}", parent_path, sub_path)
                };
                frames.extend(self.apply_path(&full_path, &sub_op, v));
            }
        }

        frames
    }

    fn apply_initial_snapshot(&mut self, response: &serde_json::Value) -> Vec<DsFrame> {
        let mut frames = Vec::new();

        // status
        if let Some(s) = response.get("status").and_then(|v| v.as_str()) {
            self.status = Some(s.to_string());
        }

        // token usage
        if let Some(n) = response
            .get("accumulated_token_usage")
            .and_then(|v| v.as_u64())
        {
            self.accumulated_token_usage = Some(u32::try_from(n).unwrap_or(u32::MAX));
        }

        // fragments
        if let Some(arr) = response.get("fragments").and_then(|f| f.as_array()) {
            self.fragments.clear();
            for frag in arr {
                let Some(ty) = frag.get("type").and_then(|t| t.as_str()) else {
                    continue;
                };
                let content = frag
                    .get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string();
                self.fragments.push(Fragment {
                    ty: ty.to_string(),
                    content: content.clone(),
                });
                if !content.is_empty() {
                    match ty {
                        FRAG_THINK => frames.push(DsFrame::ThinkDelta(content)),
                        FRAG_RESPONSE => frames.push(DsFrame::ContentDelta(content)),
                        _ => {}
                    }
                }
            }
        }

        frames
    }

    fn apply_path(&mut self, path: &str, op: &str, val: &serde_json::Value) -> Vec<DsFrame> {
        let mut frames = Vec::new();

        match path {
            "response/status" => {
                if let Some(s) = val.as_str() {
                    self.status = Some(s.to_string());
                    if s == "FINISHED" || s == "INCOMPLETE" {
                        let has_response = self
                            .fragments
                            .iter()
                            .any(|f| f.ty == "RESPONSE" && !f.content.is_empty());
                        if !has_response && s == "FINISHED" {
                            warn!(
                                target: "adapter",
                                "状态机 FINISHED 但无 RESPONSE 内容: fragments={:?}, status={:?}, accumulated_token_usage={:?}",
                                self.fragments.iter().map(|f| format!("{}/{}", f.ty, f.content.len())).collect::<Vec<_>>(),
                                self.status, self.accumulated_token_usage
                            );
                        }
                    }
                    frames.push(DsFrame::Status(s.to_string()));
                }
            }
            "response/accumulated_token_usage" | "accumulated_token_usage" => {
                if let Some(n) = val.as_u64() {
                    let u = u32::try_from(n).unwrap_or(u32::MAX);
                    self.accumulated_token_usage = Some(u);
                    frames.push(DsFrame::Usage(u));
                }
            }
            "response/fragments/-1/content" => {
                if let Some(s) = val.as_str()
                    && let Some(frag) = self.fragments.last_mut()
                {
                    match frag.ty.as_str() {
                        FRAG_THINK => {
                            frag.content.push_str(s);
                            frames.push(DsFrame::ThinkDelta(s.to_string()));
                        }
                        FRAG_RESPONSE => {
                            frag.content.push_str(s);
                            frames.push(DsFrame::ContentDelta(s.to_string()));
                        }
                        _ => {}
                    }
                }
            }
            "response/fragments" if op == "APPEND" => {
                if let Some(arr) = val.as_array() {
                    for item in arr {
                        if let Some(ty) = item.get("type").and_then(|t| t.as_str()) {
                            let content = item
                                .get("content")
                                .and_then(|c| c.as_str())
                                .unwrap_or("")
                                .to_string();
                            self.fragments.push(Fragment {
                                ty: ty.to_string(),
                                content: content.clone(),
                            });
                            if !content.is_empty() {
                                match ty {
                                    FRAG_THINK => frames.push(DsFrame::ThinkDelta(content)),
                                    FRAG_RESPONSE => frames.push(DsFrame::ContentDelta(content)),
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        frames
    }
}

pin_project! {
    // 对 SSE 事件流应用 patch 状态机的包装流
    pub struct StateStream<S> {
        #[pin]
        inner: S,
        state: DsState,
        pending: Vec<DsFrame>,
    }
}

impl<S> StateStream<S> {
    /// 创建状态流包装器
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            state: DsState::default(),
            pending: Vec::new(),
        }
    }
}

impl<S, E> Stream for StateStream<S>
where
    S: Stream<Item = Result<SseEvent, E>>,
    E: Into<OpenAIAdapterError>,
{
    type Item = Result<DsFrame, OpenAIAdapterError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        if let Some(frame) = this.pending.pop() {
            return Poll::Ready(Some(Ok(frame)));
        }

        loop {
            match this.inner.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(evt))) => {
                    let frames = this.state.apply_event(&evt);
                    if frames.is_empty() {
                        continue;
                    }
                    let mut frames = frames;
                    let first = frames.remove(0);
                    trace!(target: "adapter", ">>> state: {}", trace_frame(&first));
                    // 剩余帧按正序压入 pending（先压后出的会逆序，所以逆序 extend）
                    this.pending.extend(frames.into_iter().rev());
                    return Poll::Ready(Some(Ok(first)));
                }
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(e.into())));
                }
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

/// TRACE 日志用：截断长文本，其余变体直接 Debug
fn trace_frame(frame: &DsFrame) -> String {
    const MAX_LEN: usize = 60;
    match frame {
        DsFrame::ContentDelta(s) | DsFrame::ThinkDelta(s) => {
            let ty = if matches!(frame, DsFrame::ContentDelta(_)) {
                "ContentDelta"
            } else {
                "ThinkDelta"
            };
            if s.len() > MAX_LEN {
                format!("{}(\"{}\")", ty, &s[..MAX_LEN])
            } else {
                format!("{:?}", frame)
            }
        }
        _ => format!("{:?}", frame),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_content_with_explicit_append() {
        let mut state = DsState::default();
        state.fragments.push(Fragment {
            ty: "RESPONSE".into(),
            content: "".into(),
        });
        let evt = SseEvent {
            event: None,
            data: r#"{"p":"response/fragments/-1/content","o":"APPEND","v":"hello"}"#.into(),
        };
        let frames = state.apply_event(&evt);
        assert!(matches!(&frames[0], DsFrame::ContentDelta(s) if s == "hello"));
    }

    #[test]
    fn append_content_with_bare_v_after_path_set() {
        let mut state = DsState::default();
        state.fragments.push(Fragment {
            ty: "RESPONSE".into(),
            content: "hello".into(),
        });
        // 模拟上一事件设定了 path 和 op=APPEND
        state.current_path = Some("response/fragments/-1/content".into());
        state.current_op = Some("APPEND".into());
        let evt = SseEvent {
            event: None,
            data: r#"{"v":" world"}"#.into(),
        };
        let frames = state.apply_event(&evt);
        assert!(matches!(&frames[0], DsFrame::ContentDelta(s) if s == " world"));
    }

    #[test]
    fn snapshot_then_append() {
        let mut state = DsState::default();
        let evt = SseEvent {
            event: None,
            data: r#"{"v":{"response":{"fragments":[{"type":"THINK","content":"hi"}]}}}"#.into(),
        };
        let frames = state.apply_event(&evt);
        assert!(matches!(&frames[0], DsFrame::ThinkDelta(s) if s == "hi"));
    }

    #[test]
    fn ready_event() {
        let mut state = DsState::default();
        let frames = state.apply_event(&SseEvent {
            event: Some("ready".into()),
            data: "{}".into(),
        });
        assert!(matches!(frames[0], DsFrame::Role));
    }

    #[test]
    fn batch_accumulated_token_usage() {
        let mut state = DsState::default();
        let evt = SseEvent {
            event: None,
            data: r#"{"p":"response","o":"BATCH","v":[{"p":"accumulated_token_usage","v":41},{"p":"quasi_status","v":"FINISHED"}]}"#.into(),
        };
        let frames = state.apply_event(&evt);
        assert!(matches!(
            &frames[0],
            DsFrame::Usage(u) if *u == 41
        ));
    }

    #[test]
    fn batch_fragment_level_with_path_prepending() {
        let mut state = DsState::default();
        state.fragments.push(Fragment {
            ty: "RESPONSE".into(),
            content: "hello".into(),
        });
        let evt = SseEvent {
            event: None,
            data: r#"{"p":"response/fragments/-1","o":"BATCH","v":[{"p":"content","o":"APPEND","v":"[reference:3]"},{"p":"references","o":"SET","v":[{"id":5,"type":"TOOL_OPEN"}]}]}"#.into(),
        };
        let frames = state.apply_event(&evt);
        assert_eq!(frames.len(), 1);
        assert!(matches!(&frames[0], DsFrame::ContentDelta(s) if s == "[reference:3]"));
        assert_eq!(
            state.fragments.last().unwrap().content,
            "hello[reference:3]"
        );
    }

    #[test]
    fn batch_fragment_bare_v_array_continues_batch() {
        let mut state = DsState::default();
        state.fragments.push(Fragment {
            ty: "RESPONSE".into(),
            content: "hello world".into(),
        });
        state.current_path = Some("response/fragments/-1".into());
        state.current_op = Some("BATCH".into());
        let evt = SseEvent {
            event: None,
            data: r#"{"v":[{"p":"content","o":"APPEND","v":"[reference:1]"},{"p":"references","v":[{"id":6,"type":"TOOL_OPEN"}]}]}"#.into(),
        };
        let frames = state.apply_event(&evt);
        assert_eq!(frames.len(), 1);
        assert!(matches!(&frames[0], DsFrame::ContentDelta(s) if s == "[reference:1]"));
        assert_eq!(
            state.fragments.last().unwrap().content,
            "hello world[reference:1]"
        );
    }

    #[test]
    fn incomplete_status_with_finish_event() {
        let mut state = DsState::default();
        let evt = SseEvent {
            event: None,
            data: r#"{"p":"response/status","v":"INCOMPLETE"}"#.into(),
        };
        let frames = state.apply_event(&evt);
        assert_eq!(frames.len(), 1);
        assert!(matches!(&frames[0], DsFrame::Status(s) if s == "INCOMPLETE"));
    }

    #[test]
    fn batch_decomposition_preserves_outer_path() {
        // BATCH 结束后，外层 path/op 保持原值
        let mut state = DsState::default();
        // 先模拟一段正常对话：设 path+op，然后 BATCH，确保 BATCH 结束后 path/op 恢复
        state.current_path = Some("response/fragments/-1".into());
        state.current_op = Some("BATCH".into());
        let evt = SseEvent {
            event: None,
            data: r#"{"v":[{"p":"content","o":"APPEND","v":"x"}]}"#.into(),
        };
        state.apply_event(&evt);
        assert_eq!(state.current_path.as_deref(), Some("response/fragments/-1"));
        assert_eq!(state.current_op.as_deref(), Some("BATCH"));
    }

    #[test]
    fn complex_tool_search_with_think_and_response() {
        let mut state = DsState::default();

        // 初始快照：THINK fragment
        let evt = SseEvent {
            event: None,
            data: r#"{"v":{"response":{"fragments":[{"type":"THINK","content":"思考"}]}}}"#.into(),
        };
        state.apply_event(&evt);
        assert_eq!(state.fragments.len(), 1);

        // TOOL_SEARCH APPEND
        let evt = SseEvent {
            event: None,
            data: r#"{"p":"response/fragments","o":"APPEND","v":[{"id":3,"type":"TOOL_SEARCH","content":null,"queries":[{"query":"q"}],"results":[]}]}"#.into(),
        };
        let frames = state.apply_event(&evt);
        assert!(frames.is_empty()); // TOOL_SEARCH 不产生可见内容
        assert_eq!(state.fragments.len(), 2);
        assert_eq!(state.fragments[1].ty, "TOOL_SEARCH");

        // TOOL_OPEN APPEND
        let evt = SseEvent {
            event: None,
            data: r#"{"p":"response/fragments","o":"APPEND","v":[{"id":4,"type":"TOOL_OPEN","status":"WIP","result":{"url":"https://x.com","title":"t","snippet":"s"},"reference":{"id":3,"type":"TOOL_SEARCH"}}]}"#.into(),
        };
        let frames = state.apply_event(&evt);
        assert!(frames.is_empty());
        assert_eq!(state.fragments.len(), 3);

        // 新 THINK APPEND
        let evt = SseEvent {
            event: None,
            data:
                r#"{"p":"response/fragments","o":"APPEND","v":[{"type":"THINK","content":"继续"}]}"#
                    .into(),
        };
        let frames = state.apply_event(&evt);
        assert_eq!(frames.len(), 1);
        assert!(matches!(&frames[0], DsFrame::ThinkDelta(s) if s == "继续"));
        assert_eq!(state.fragments.len(), 4);

        // RESPONSE APPEND
        let evt = SseEvent {
            event: None,
            data:
                r#"{"p":"response/fragments","o":"APPEND","v":[{"type":"RESPONSE","content":""}]}"#
                    .into(),
        };
        let frames = state.apply_event(&evt);
        assert!(frames.is_empty());
        assert_eq!(state.fragments.len(), 5);

        // RESPONSE content
        let evt = SseEvent {
            event: None,
            data: r#"{"p":"response/fragments/-1/content","o":"APPEND","v":"hello"}"#.into(),
        };
        let frames = state.apply_event(&evt);
        assert!(matches!(&frames[0], DsFrame::ContentDelta(s) if s == "hello"));

        // FINISHED
        let evt = SseEvent {
            event: None,
            data: r#"{"p":"response/status","v":"FINISHED"}"#.into(),
        };
        let frames = state.apply_event(&evt);
        assert!(matches!(&frames[0], DsFrame::Status(s) if s == "FINISHED"));
    }
}
