//! Wire protocol types and the task state machine shared by node, client and GUI.

use serde::{Deserialize, Serialize};

/// Wire protocol version announced by health/capabilities and mDNS.
pub const PROTOCOL_VERSION: u32 = 1;

/// mDNS/DNS-SD service type advertised by every node.
pub const MDNS_SERVICE_TYPE: &str = "_localtranscribe._tcp.local.";

/// TXT record keys carried in the mDNS announcement.
pub const TXT_VER: &str = "ver";
pub const TXT_NAME: &str = "name";
pub const TXT_COMPUTE: &str = "compute";
pub const TXT_ROLE: &str = "role";
pub const TXT_MODEL: &str = "model";

/// Value of the `compute` TXT key when the node transcribes locally.
pub const TXT_COMPUTE_ON: &str = "on";
/// Value of the `compute` TXT key when the node only stores and forwards.
pub const TXT_COMPUTE_OFF: &str = "off";

/// Largest accepted upload chunk. Also advertised in [`Capabilities`].
pub const MAX_UPLOAD_CHUNK_BYTES: u64 = 64 * 1024 * 1024;

/// One task in the node-local task store.
///
/// Serialized with snake_case field names as the HTTP wire format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub state: TaskState,
    pub file_name: String,
    pub file_size: u64,
    pub received_bytes: u64,
    pub chunks_total: u32,
    pub chunks_received: u32,
    pub model_id: Option<String>,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
    pub error: Option<TaskError>,
    pub result: Option<TaskResult>,
}

impl Task {
    /// Build a fresh task in `Init` state; the node moves it to
    /// `FileTransferring` as soon as it is accepted.
    pub fn new(
        id: String,
        file_name: String,
        file_size: u64,
        chunks_total: u32,
        model_id: Option<String>,
        now_unix_ms: u64,
    ) -> Self {
        Self {
            id,
            state: TaskState::Init,
            file_name,
            file_size,
            received_bytes: 0,
            chunks_total,
            chunks_received: 0,
            model_id,
            created_at_unix_ms: now_unix_ms,
            updated_at_unix_ms: now_unix_ms,
            error: None,
            result: None,
        }
    }
}

/// Task life-cycle states. The only legal moves are the ones
/// [`TaskState::can_transition_to`] accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Init,
    FileTransferring,
    Queued,
    Transcribing,
    PostProcessing,
    Completed,
    Failed,
    CancelRequested,
    Cancelled,
}

impl TaskState {
    /// Whether `self -> next` is a legal transition.
    ///
    /// Legal edges: Init -> FileTransferring -> Queued -> Transcribing ->
    /// PostProcessing -> Completed; Queued/Transcribing/PostProcessing -> Failed;
    /// Queued/Transcribing -> CancelRequested -> Cancelled. A task that was
    /// cancelled but already finished wrapping up may end as Completed.
    pub fn can_transition_to(&self, next: &Self) -> bool {
        use TaskState::*;
        matches!(
            (*self, *next),
            (Init, FileTransferring)
                | (FileTransferring, Queued)
                | (Queued, Transcribing)
                | (Transcribing, PostProcessing)
                | (PostProcessing, Completed)
                | (Queued, Failed)
                | (Transcribing, Failed)
                | (PostProcessing, Failed)
                | (Queued, CancelRequested)
                | (Transcribing, CancelRequested)
                | (CancelRequested, Cancelled)
                | (CancelRequested, Completed)
        )
    }

    /// Whether the task reached a state it can never leave.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskState::Completed | TaskState::Failed | TaskState::Cancelled
        )
    }

    /// Stable snake_case name used in messages and wire payloads.
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskState::Init => "init",
            TaskState::FileTransferring => "file_transferring",
            TaskState::Queued => "queued",
            TaskState::Transcribing => "transcribing",
            TaskState::PostProcessing => "post_processing",
            TaskState::Completed => "completed",
            TaskState::Failed => "failed",
            TaskState::CancelRequested => "cancel_requested",
            TaskState::Cancelled => "cancelled",
        }
    }
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Plain-text transcript plus the aligned segments produced by the engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskResult {
    pub transcript: String,
    pub segments: Vec<Segment>,
    pub duration_ms: u64,
}

/// One timed stretch of recognized text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

/// Failure attached to a task; never carries secrets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskError {
    pub code: String,
    pub message: String,
}

/// A node as seen through discovery or direct contact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeInfo {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub proto_version: u32,
    pub compute_enabled: bool,
    pub role: String,
    pub model_id: Option<String>,
}

/// What a node can do, reported by `GET /api/v1/capabilities`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteModel {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    pub proto_version: u32,
    pub max_chunk_bytes: u64,
    pub compute_enabled: bool,
    pub role: String,
    pub model_id: Option<String>,
    pub platform: String,
    /// Display name of the node.
    ///
    /// Added for TCP fallback discovery, where there is no mDNS TXT record to
    /// read the name from. Nodes built before this field simply omit it, so
    /// deserialization defaults to an empty string and the client falls back
    /// to the address.
    #[serde(default)]
    pub name: String,
}

/// A node registering itself or updating its display name and role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub name: String,
    pub role: String,
}

/// Upload progress body returned by the chunked upload endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadProgress {
    pub received_bytes: u64,
    pub chunks_received: u32,
}

/// Result of `POST /api/v1/collect`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CollectReport {
    pub exported: Vec<String>,
    pub failed: Vec<(String, String)>,
}

/// Result of `GET /api/v1/storage`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageInfo {
    pub data_dir: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
}

/// Error body used by every non-2xx node response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(from: TaskState, to: TaskState) -> (TaskState, TaskState) {
        (from, to)
    }

    #[test]
    fn happy_path_edges_are_allowed() {
        use TaskState::*;
        for (from, to) in [
            edge(Init, FileTransferring),
            edge(FileTransferring, Queued),
            edge(Queued, Transcribing),
            edge(Transcribing, PostProcessing),
            edge(PostProcessing, Completed),
        ] {
            assert!(
                from.can_transition_to(&to),
                "{from:?} -> {to:?} must be allowed"
            );
        }
    }

    #[test]
    fn failure_edges_are_allowed_from_running_states() {
        use TaskState::*;
        for (from, to) in [
            edge(Queued, Failed),
            edge(Transcribing, Failed),
            edge(PostProcessing, Failed),
        ] {
            assert!(
                from.can_transition_to(&to),
                "{from:?} -> {to:?} must be allowed"
            );
        }
    }

    #[test]
    fn cancel_edges_are_allowed() {
        use TaskState::*;
        for (from, to) in [
            edge(Queued, CancelRequested),
            edge(Transcribing, CancelRequested),
            edge(CancelRequested, Cancelled),
        ] {
            assert!(
                from.can_transition_to(&to),
                "{from:?} -> {to:?} must be allowed"
            );
        }
    }

    #[test]
    fn cancel_requested_can_finish_as_completed_when_wrap_up_succeeded() {
        assert!(TaskState::CancelRequested.can_transition_to(&TaskState::Completed));
    }

    #[test]
    fn skipping_or_repeating_steps_is_rejected() {
        use TaskState::*;
        for (from, to) in [
            edge(Init, Queued),
            edge(Init, Transcribing),
            edge(Init, Completed),
            edge(Init, Failed),
            edge(Init, CancelRequested),
            edge(FileTransferring, Transcribing),
            edge(FileTransferring, PostProcessing),
            edge(FileTransferring, Completed),
            edge(FileTransferring, Failed),
            edge(FileTransferring, CancelRequested),
            edge(Queued, PostProcessing),
            edge(Queued, Completed),
            edge(Transcribing, Completed),
            edge(Transcribing, Init),
            edge(PostProcessing, Transcribing),
            edge(PostProcessing, CancelRequested),
        ] {
            assert!(
                !from.can_transition_to(&to),
                "{from:?} -> {to:?} must be rejected"
            );
        }
    }

    #[test]
    fn terminal_states_have_no_outgoing_edges() {
        use TaskState::*;
        let terminals = [Completed, Failed, Cancelled];
        let all = [
            Init,
            FileTransferring,
            Queued,
            Transcribing,
            PostProcessing,
            Completed,
            Failed,
            CancelRequested,
            Cancelled,
        ];
        for from in terminals {
            for to in all {
                assert!(
                    !from.can_transition_to(&to),
                    "{from:?} -> {to:?} must be rejected"
                );
            }
        }
    }

    #[test]
    fn self_transitions_are_rejected() {
        for state in [
            TaskState::Init,
            TaskState::FileTransferring,
            TaskState::Queued,
            TaskState::Transcribing,
            TaskState::PostProcessing,
            TaskState::Completed,
            TaskState::Failed,
            TaskState::CancelRequested,
            TaskState::Cancelled,
        ] {
            assert!(!state.can_transition_to(&state), "{state:?} -> itself");
        }
    }

    #[test]
    fn task_state_serializes_snake_case() {
        let cases = [
            (TaskState::Init, "\"init\""),
            (TaskState::FileTransferring, "\"file_transferring\""),
            (TaskState::Queued, "\"queued\""),
            (TaskState::Transcribing, "\"transcribing\""),
            (TaskState::PostProcessing, "\"post_processing\""),
            (TaskState::Completed, "\"completed\""),
            (TaskState::Failed, "\"failed\""),
            (TaskState::CancelRequested, "\"cancel_requested\""),
            (TaskState::Cancelled, "\"cancelled\""),
        ];
        for (state, expected) in cases {
            let json = serde_json::to_string(&state).expect("serialize state");
            assert_eq!(json, expected);
            let back: TaskState = serde_json::from_str(&json).expect("deserialize state");
            assert_eq!(back, state);
        }
    }

    #[test]
    fn task_round_trips_through_json() {
        let task = Task {
            id: "0b0f7b6a-58f0-4f0e-9c22-6b1a2b6a0e11".to_owned(),
            state: TaskState::Transcribing,
            file_name: "talk.mp3".to_owned(),
            file_size: 4096,
            received_bytes: 4096,
            chunks_total: 2,
            chunks_received: 2,
            model_id: Some("sensevoice-small".to_owned()),
            created_at_unix_ms: 1,
            updated_at_unix_ms: 2,
            error: Some(TaskError {
                code: "interrupted".to_owned(),
                message: "服务中断，任务未完成".to_owned(),
            }),
            result: Some(TaskResult {
                transcript: "你好".to_owned(),
                segments: vec![Segment {
                    start_ms: 0,
                    end_ms: 900,
                    text: "你好".to_owned(),
                }],
                duration_ms: 900,
            }),
        };
        let json = serde_json::to_string(&task).expect("serialize task");
        let back: Task = serde_json::from_str(&json).expect("deserialize task");
        assert_eq!(back, task);
        assert!(json.contains("\"file_name\":\"talk.mp3\""));
        assert!(json.contains("\"start_ms\":0"));
    }
}
