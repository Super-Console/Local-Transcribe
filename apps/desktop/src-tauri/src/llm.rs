//! Local, on-demand language-model commands for the desktop UI.
//!
//! Generation is emitted as Tauri events so tokens can be displayed as they
//! arrive without opening a network listener. The same runtime is used by the
//! node API in `voice-to-text-lan`.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use voice_to_text_lan::llm::{LlmRuntime, LlmStatus, select_references, split_reference_segments};

pub const EVENT: &str = "voice-to-text:llm-event";
const MAX_TEXT_CHARS: usize = 500_000;
const MAX_QUESTION_CHARS: usize = 2_000;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationRequest {
    pub action: String,
    pub model_id: Option<String>,
    pub text: String,
    pub selection: Option<String>,
    pub question: Option<String>,
    /// 界面语言（BCP-47）：答案必须跟随它，而不是跟随转写内容的语言。
    pub language: Option<String>,
    #[serde(default)]
    pub history: Vec<AskTurn>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AskTurn {
    pub question: String,
    pub answer: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerationEvent {
    request_id: String,
    kind: &'static str,
    data: serde_json::Value,
}

#[tauri::command]
pub fn status(runtime: State<'_, Arc<LlmRuntime>>) -> LlmStatus {
    runtime.status()
}

#[tauri::command]
pub async fn load(
    model_id: String,
    runtime: State<'_, Arc<LlmRuntime>>,
) -> Result<LlmStatus, String> {
    let runtime = Arc::clone(runtime.inner());
    tauri::async_runtime::spawn_blocking(move || runtime.load(&model_id))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn unload(runtime: State<'_, Arc<LlmRuntime>>) -> Result<LlmStatus, String> {
    let runtime = Arc::clone(runtime.inner());
    tauri::async_runtime::spawn_blocking(move || {
        runtime.unload();
        runtime.status()
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn cancel(runtime: State<'_, Arc<LlmRuntime>>) -> bool {
    runtime.cancel_generation()
}

#[tauri::command]
pub fn start_generation(
    request: GenerationRequest,
    app: AppHandle,
    runtime: State<'_, Arc<LlmRuntime>>,
) -> Result<String, String> {
    validate_request(&request)?;
    let (system_prompt, user_prompt) = build_prompt(&request)?;
    let model_id = request
        .model_id
        .clone()
        .unwrap_or_else(|| "qwen3-4b".to_owned());
    let request_id = uuid::Uuid::new_v4().to_string();
    let runtime = Arc::clone(runtime.inner());
    let thread_request_id = request_id.clone();
    let generation_id = runtime.prepare_generation()?;
    let abort_runtime = Arc::clone(&runtime);

    let spawned = std::thread::Builder::new()
        .name(format!("llm-{}", &request_id[..8]))
        .spawn(move || {
            let progress_app = app.clone();
            let progress_request_id = thread_request_id.clone();
            let stage_app = app.clone();
            let stage_request_id = thread_request_id.clone();
            let partial_app = app.clone();
            let partial_request_id = thread_request_id.clone();
            let token_app = app.clone();
            let token_request_id = thread_request_id.clone();
            let result = runtime.generate_segmented_stream(
                generation_id,
                &model_id,
                &system_prompt,
                &user_prompt,
                &request.text,
                |segment| segment_user_prompt(&request, segment),
                |partials| merge_user_prompt(&request, partials),
                Arc::new(move |progress| {
                    emit(
                        &progress_app,
                        &progress_request_id,
                        "progress",
                        serde_json::json!({ "phase": "loading", "progress": progress }),
                    )
                }),
                move |phase, current, total| {
                    emit(
                        &stage_app,
                        &stage_request_id,
                        "stage",
                        serde_json::json!({
                            "phase": phase,
                            "current": current,
                            "total": total
                        }),
                    )
                },
                |token| {
                    emit(
                        &partial_app,
                        &partial_request_id,
                        "partial_token",
                        serde_json::Value::String(token.to_owned()),
                    )
                },
                |token| {
                    emit(
                        &token_app,
                        &token_request_id,
                        "token",
                        serde_json::Value::String(token.to_owned()),
                    )
                },
            );

            match result {
                Ok(completed) => {
                    if completed && request.action == "ask" {
                        let segments = split_reference_segments(&request.text);
                        let references = select_references(
                            &segments,
                            request.question.as_deref().unwrap_or_default(),
                        );
                        let _ = emit(
                            &app,
                            &thread_request_id,
                            "references",
                            serde_json::json!({ "references": references }),
                        );
                    }
                    let _ = emit(
                        &app,
                        &thread_request_id,
                        "done",
                        serde_json::json!({ "cancelled": !completed }),
                    );
                }
                Err(message) => {
                    let _ = emit(
                        &app,
                        &thread_request_id,
                        "error",
                        serde_json::json!({
                            "code": error_code(&message),
                            "message": message
                        }),
                    );
                }
            }
        });
    if let Err(error) = spawned {
        abort_runtime.abort_generation(generation_id);
        return Err(format!(
            "Could not start language-model generation: {error}"
        ));
    }

    Ok(request_id)
}

/// 进程退出前必须执行的 LLM 清理。
///
/// tao 在事件循环结束后直接调用 `std::process::exit`，Rust 托管状态因此
/// 不会被 drop。macOS 上残留的 llama/ggml Metal 资源会让 ggml 的静态析构
/// 命中 `GGML_ASSERT([rsets->data count] == 0)`（`ggml-metal-device.m:1040`）
/// 并 abort，所以退出路径必须显式取消生成并卸载模型。
pub(crate) fn shutdown_for_exit<R: tauri::Runtime>(app: &AppHandle<R>) {
    if let Some(runtime) = app.try_state::<Arc<LlmRuntime>>() {
        // 先置取消位，让生成线程在当前 token 边界退出；unload 随后阻塞等待
        // 它释放 operation 锁，再同步 drop 模型与 backend，Metal 资源即被释放。
        runtime.cancel_generation();
        runtime.unload();
    }
}

fn emit(app: &AppHandle, request_id: &str, kind: &'static str, data: serde_json::Value) -> bool {
    app.emit(
        EVENT,
        GenerationEvent {
            request_id: request_id.to_owned(),
            kind,
            data,
        },
    )
    .is_ok()
}

fn validate_request(request: &GenerationRequest) -> Result<(), String> {
    if request.text.trim().is_empty() {
        return Err("Provide the transcription text first.".to_owned());
    }
    if request.text.chars().count() > MAX_TEXT_CHARS {
        return Err(format!(
            "The supplied transcription exceeds the {MAX_TEXT_CHARS}-character limit."
        ));
    }
    match request.action.as_str() {
        "summary" | "points" | "todos" => Ok(()),
        "explain" => {
            if request
                .selection
                .as_deref()
                .is_none_or(|selection| selection.trim().is_empty())
            {
                Err("Select a passage in the transcription to explain.".to_owned())
            } else if request
                .selection
                .as_deref()
                .is_some_and(|selection| selection.chars().count() > 20_000)
            {
                Err("The selected passage exceeds the 20,000-character limit.".to_owned())
            } else {
                Ok(())
            }
        }
        "ask" => {
            if request.history.len() > 3 {
                return Err(
                    "Ask history is limited to three turns. Clear it before continuing.".to_owned(),
                );
            }
            let question = request.question.as_deref().unwrap_or_default().trim();
            if question.is_empty() {
                return Err("Enter a question.".to_owned());
            }
            if question.chars().count() > MAX_QUESTION_CHARS {
                return Err(format!(
                    "The question exceeds the {MAX_QUESTION_CHARS}-character limit."
                ));
            }
            Ok(())
        }
        _ => Err("Choose summary, points, todos, explain, or ask.".to_owned()),
    }
}

/// 界面语言 → 模型可执行的目标语言指令。只映射应用实际发布的语言；
/// 未知语言（含未来新增但未同步的）不强加指令，避免模型收到无法解析的名称。
fn answer_language_directive(language: Option<&str>) -> &'static str {
    match language {
        Some("zh-CN") => "Write the answer in Simplified Chinese.",
        Some("en") => "Write the answer in English.",
        Some("es") => "Write the answer in Spanish.",
        Some("fr") => "Write the answer in French.",
        Some("de") => "Write the answer in German.",
        _ => "",
    }
}

fn build_prompt(request: &GenerationRequest) -> Result<(String, String), String> {
    let policy = "Use only the supplied transcription. Do not invent names, numbers, events, or conclusions absent from it. If the text does not contain the answer, say so.";
    let language = answer_language_directive(request.language.as_deref());
    let (instruction, source) = match request.action.as_str() {
        "summary" => (
            "Write a concise summary of the transcription.",
            request.text.as_str(),
        ),
        "points" => (
            "Extract the key points as a short bullet list.",
            request.text.as_str(),
        ),
        "todos" => (
            "List only explicit action items and their stated owners or deadlines. If none are present, say there are no explicit action items.",
            request.text.as_str(),
        ),
        "explain" => (
            "Explain the selected passage using only the surrounding transcription.",
            request.text.as_str(),
        ),
        "ask" => {
            let system = format!(
                "Answer only from the supplied transcription. Do not invent names, numbers, events, or conclusions. If the text does not contain the answer, say so. Keep the answer concise and identify supporting wording with short verbatim quotes when possible. {policy} {language}"
            );
            return Ok((
                system.trim_end().to_owned(),
                segment_user_prompt(request, &request.text),
            ));
        }
        _ => return Err("Choose summary, points, todos, explain, or ask.".to_owned()),
    };
    let user_prompt = segment_user_prompt(request, source);
    Ok((
        format!("{instruction} {policy} {language}")
            .trim_end()
            .to_owned(),
        user_prompt,
    ))
}

fn segment_user_prompt(request: &GenerationRequest, transcript: &str) -> String {
    if request.action == "ask" {
        let history = request
            .history
            .iter()
            .take(3)
            .enumerate()
            .map(|(index, turn)| {
                format!(
                    "Earlier turn {} — question: {}\nEarlier answer: {}\n",
                    index + 1,
                    turn.question
                        .chars()
                        .take(MAX_QUESTION_CHARS)
                        .collect::<String>(),
                    turn.answer.chars().take(4_000).collect::<String>()
                )
            })
            .collect::<String>();
        return format!(
            "TRANSCRIPTION:\n{transcript}\n\n{history}QUESTION:\n{}",
            request.question.as_deref().unwrap_or_default().trim()
        );
    }
    if request.action == "explain" {
        return format!(
            "TRANSCRIPTION:\n{transcript}\n\nSELECTED PASSAGE:\n{}",
            request.selection.as_deref().unwrap_or_default().trim()
        );
    }
    format!("TRANSCRIPTION:\n{transcript}")
}

fn merge_user_prompt(request: &GenerationRequest, partials: &str) -> String {
    let mut prompt = format!("PARTIAL RESULTS FROM TRANSCRIPTION SEGMENTS:\n{partials}");
    if request.action == "ask" {
        prompt.push_str(&format!(
            "\n\nQUESTION:\n{}",
            request.question.as_deref().unwrap_or_default().trim()
        ));
    } else if request.action == "explain" {
        prompt.push_str(&format!(
            "\n\nSELECTED PASSAGE:\n{}",
            request.selection.as_deref().unwrap_or_default().trim()
        ));
    }
    prompt.push_str("\n\nCombine the partial results into one concise, coherent answer. Do not add facts absent from the partial results.");
    prompt
}

fn error_code(message: &str) -> &'static str {
    if message.contains("not installed") {
        "model_not_installed"
    } else if message.contains("Insufficient memory") {
        "insufficient_memory"
    } else if message.contains("cancelled") {
        "cancelled"
    } else if message.contains("busy") {
        "llm_busy"
    } else {
        "llm_failed"
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AskTurn, GenerationRequest, build_prompt, merge_user_prompt, segment_user_prompt,
        select_references, shutdown_for_exit, split_reference_segments, validate_request,
    };
    use std::sync::Arc;
    use tauri::Manager;
    use voice_to_text_lan::llm::LlmRuntime;

    #[test]
    fn exit_shutdown_cancels_the_generation_and_unloads_the_model() {
        let app = tauri::test::mock_app();
        let runtime = Arc::new(
            LlmRuntime::new(std::env::temp_dir().join("voice-to-text-exit-test"), 120)
                .expect("runtime"),
        );
        assert!(app.manage(Arc::clone(&runtime)));
        runtime.prepare_generation().expect("generation starts");
        assert!(
            runtime.status().busy,
            "precondition: a generation is active"
        );

        shutdown_for_exit(app.handle());

        let status = runtime.status();
        assert!(!status.busy, "exit must cancel the active generation");
        assert!(!status.loaded, "exit must leave no model loaded");
        assert!(
            !runtime.cancel_generation(),
            "no generation may stay active after the exit cleanup"
        );
    }

    fn request(action: &str, text: &str) -> GenerationRequest {
        GenerationRequest {
            action: action.to_owned(),
            model_id: None,
            text: text.to_owned(),
            selection: None,
            question: None,
            language: None,
            history: Vec::new(),
        }
    }

    #[test]
    fn summary_prompt_is_grounded_in_transcription() {
        let request = request("summary", "今天在北京开会，预算是三十万元。");
        let (system, user) = build_prompt(&request).unwrap();
        assert!(system.contains("Do not invent names, numbers"));
        assert!(user.contains("预算是三十万元"));
    }

    /// 答案语言跟随界面语言，而不是默认英文（2026-10-25 中文界面输出英文摘要的缺陷）。
    #[test]
    fn prompts_answer_in_the_ui_language() {
        let mut request = request("summary", "今天在北京开会。");
        request.language = Some("zh-CN".to_owned());
        let (system, _) = build_prompt(&request).unwrap();
        assert!(
            system.contains("Simplified Chinese"),
            "missing language directive: {system}"
        );
    }

    /// 未知或缺失语言不能凭空捏造目标语言，保持原有行为。
    #[test]
    fn unknown_or_missing_language_adds_no_directive() {
        for language in [None, Some("tlh".to_owned())] {
            let mut request = request("summary", "Today we met in Beijing.");
            request.language = language;
            let (system, _) = build_prompt(&request).unwrap();
            assert!(!system.contains("Simplified Chinese"));
            assert!(!system.contains("Answer in"));
        }
    }

    #[test]
    fn asks_keep_at_most_three_bounded_history_turns_and_references() {
        let mut request = request("ask", "项目在上海进行，预算为 30 万元。负责人是林青。");
        request.question = Some("谁负责？".to_owned());
        request.history = (0..5)
            .map(|index| AskTurn {
                question: format!("问题 {index}"),
                answer: format!("回答 {index}"),
            })
            .collect();
        let (_, user) = build_prompt(&request).unwrap();
        assert!(user.contains("Earlier turn 3"));
        assert!(!user.contains("Earlier turn 4"));
        let segments = split_reference_segments(&request.text);
        let references = select_references(&segments, "上海预算");
        assert_eq!(references.len(), 1);
        assert!(references[0].text.contains("上海进行"));
    }

    #[test]
    fn explain_requires_a_selected_passage() {
        assert!(validate_request(&request("explain", "完整原文")).is_err());
        let mut request = request("explain", "完整原文");
        request.selection = Some("原文".to_owned());
        assert!(validate_request(&request).is_ok());
        let (_, user) = build_prompt(&request).unwrap();
        assert!(user.contains("TRANSCRIPTION:\n完整原文"));
        assert!(user.contains("SELECTED PASSAGE:\n原文"));
    }

    #[test]
    fn segmented_ask_prompt_keeps_the_question_and_merge_uses_only_partial_results() {
        let mut request = request("ask", "完整转写");
        request.question = Some("预算是多少？".to_owned());
        request.history.push(AskTurn {
            question: "上一个问题".to_owned(),
            answer: "已有回答".to_owned(),
        });
        let segment = segment_user_prompt(&request, "这一段原文");
        assert!(segment.contains("TRANSCRIPTION:\n这一段原文"));
        assert!(!segment.contains("完整转写"));
        assert!(segment.contains("QUESTION:\n预算是多少？"));
        assert!(segment.contains("已有回答"));

        let merge = merge_user_prompt(&request, "部分结论一\n部分结论二");
        assert!(merge.contains("PARTIAL RESULTS FROM TRANSCRIPTION SEGMENTS"));
        assert!(merge.contains("预算是多少？"));
        assert!(merge.contains("Do not add facts absent from the partial results"));
        assert!(!merge.contains("完整转写"));
    }

    #[test]
    fn ask_history_is_limited_to_three_turns() {
        let mut request = request("ask", "原文");
        request.question = Some("问题".to_owned());
        request.history = (0..4)
            .map(|index| AskTurn {
                question: format!("问题 {index}"),
                answer: format!("回答 {index}"),
            })
            .collect();
        assert!(validate_request(&request).is_err());
    }
}
