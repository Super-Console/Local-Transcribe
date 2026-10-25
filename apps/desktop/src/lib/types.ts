/** Shared frontend types mirroring the Rust command/event payloads. */

export type View = "home" | "record" | "model" | "lan" | "settings";

export type InputFile = { path: string; fileName: string; sizeBytes: number };

export type AudioApp = { pid: number; name: string };

export type RecordedTrack = { path: string; label: string };

export type RecordingTrackLevel = { label: string; level: number; error?: string | null };

export type RecordingStatus = {
  running: boolean;
  elapsedMs: number;
  tracks: RecordingTrackLevel[];
};

/** 提交给后端的音轨：kind = input（设备）或 app（应用）。 */
export type TrackSpec = { kind: "input" | "app"; target: string; label: string };

export type ModelSummary = { id: string; displayName: string; family: string };

export type Segment = { startMs: number; endMs: number; text: string };

export type LibraryModelSummary = {
  id: string;
  displayName: string;
  family: string;
  languages?: string | null;
  bytes: number;
  estimatedMemoryBytes: number;
  selected: boolean;
};

export type CatalogModel = {
  id: string;
  name: string;
  directory: string;
  family: string;
  kind: "asr" | "punctuation" | "llm";
  tier: "quality" | "speed" | "punctuation" | "language";
  languages: string;
  bytes: number;
  installed: boolean;
  estimatedMemoryBytes: number;
};

export type ModelDownloadEvent = {
  id: string;
  state: "downloading" | "installing" | "done" | "error" | "cancelled";
  received: number;
  total: number;
  error?: string;
  libraryId?: string;
};

export type AppPermissionStatus = {
  microphone: "authorized" | "denied" | "restricted" | "not_determined" | "unknown";
  screenCapture: "granted" | "denied" | "unknown";
};

export type LanDiagnostics = {
  access: "granted" | "denied" | "no_network";
  errno: number | null;
  gateway: string | null;
  tcpResult: string;
  tcpErrno: number | null;
  verdict: "ok" | "multicast_blocked" | "no_network" | "permission";
  interfaces: LanHostInterface[];
  version: string;
  identifier: string;
  executable: string;
};

export type LanHostRetention = {
  mode: "off" | "age" | "size";
  ageDays: number;
  maxBytes: number;
};

export type StorageSummary = {
  libraryPath: string;
  libraryBytes: number;
  libraryCount: number;
  historyPath: string;
  historyBytes: number;
  historyCount: number;
  jobsPath: string;
  jobsBytes: number;
  jobsCount: number;
  lanHostPath: string;
  lanHostBytes: number;
  lanHostCount: number;
  logsPath: string;
  logsBytes: number;
  dataPath: string;
};

/** 模型运行时状态：loadedModelId 非空才代表模型真的驻留内存。 */
export type ModelRuntime = {
  loadedModelId?: string | null;
  idleUnloadSeconds: number;
};

/** 历史与任务记录的自动清理策略；默认关闭。 */
export type RetentionPolicy = {
  mode: "off" | "age" | "size";
  ageDays: number;
  maxBytes: number;
};

export type ServiceStatus = {
  running: boolean;
  pid?: number;
  socketPath: string;
  dataDir: string;
  protocolVersion: number;
};

export type JobStatus = {
  jobId: string;
  state: "running" | "completed" | "failed" | "cancelled";
  phase: string;
  completedChunks: number;
  totalChunks?: number;
  startedAtMs?: number;
  totalDurationMs?: number;
  transcriptAvailable: boolean;
  resultSaved: boolean;
  error?: string;
};

export type Transcript = {
  schemaVersion: string;
  jobId: string;
  sourceFileName: string;
  modelId: string;
  modelVersion?: string;
  engine: string;
  language?: string;
  durationMs?: number;
  text: string;
  segments?: Segment[];
};

export type SearchHit = {
  jobId: string;
  sourceFileName: string;
  modelId: string;
  durationMs?: number;
  snippet: string;
  matchedInContent: boolean;
  updatedAtMs: number;
};

export type HistoryEntry = {
  jobId: string;
  sourceFileName: string;
  modelId: string;
  durationMs?: number;
  preview: string;
  updatedAtMs: number;
};

export type SegmentEvent = {
  jobId: string;
  index: number;
  totalChunks: number;
  startMs: number;
  endMs: number;
  text: string;
};

/* ------------------------------------------------------------------ */
/* 局域网 Compute V1.1（Client 端）：与 Rust lan.rs / LAN 协议的        */
/* snake_case 载荷一一对应。                                          */
/* ------------------------------------------------------------------ */

export type LanMode = "local_only" | "prefer_lan";

export type LanHostInterface = {
  name: string;
  address: string;
  mask: string;
  kind: "lan" | "vpn" | "tailscale" | "other";
};
export type LanHostStatus = {
  supported: boolean;
  running: boolean;
  name: string;
  address: string;
  port: number;
  model: string;
  mask: string;
  subnet_filter: "same_subnet" | "none";
  notice?: string | null;
};

export type LanNodeConfig = {
  id: string;
  name: string;
  host: string;
  port: number;
  model_id?: string | null;
  enabled: boolean;
  /** 远程启停的 SSH 目标；缺省用 host。 */
  ssh?: string | null;
  /** 远程节点二进制路径；缺省按 sidecar 路径推断。 */
  node_binary?: string | null;
  /** 添加时抓取到的能力快照。 */
  capabilities?: unknown | null;
};

export type LanSettings = {
  mode: LanMode;
  nodes: LanNodeConfig[];
  idle_unload_gui: number;
  idle_unload_node: number;
  share_on_launch: boolean;
  lan_consent_acknowledged: boolean;
  keep_running_in_background: boolean;
  host_name: string;
  host_address: string;
  host_port: number;
};

export type LanNodeInfo = {
  name: string;
  host: string;
  port: number;
  proto_version: number;
  compute_enabled: boolean;
  role: string;
  model_id: string | null;
};

export type LanCapabilities = {
  proto_version: number;
  max_chunk_bytes: number;
  compute_enabled: boolean;
  role: string;
  model_id: string | null;
  platform: string;
};

export type LanNodeStatus = {
  models: { id: string; display_name: string }[];
  reachable: boolean;
  capabilities: LanCapabilities | null;
  latency_ms: number | null;
  /** Storage probe result; independent from reachability. */
  storage: LanStorageInfo | null;
  /** Non-fatal probe problem (e.g. storage unavailable). */
  warning: string | null;
};

export type LanTaskState =
  | "init"
  | "file_transferring"
  | "queued"
  | "transcribing"
  | "post_processing"
  | "completed"
  | "failed"
  | "cancel_requested"
  | "cancelled";

export type LanTaskError = { code: string; message: string };

export type LanSegment = { start_ms: number; end_ms: number; text: string };

export type LanTaskResult = {
  transcript: string;
  segments: LanSegment[];
  duration_ms: number;
};

export type LanTask = {
  id: string;
  state: LanTaskState;
  file_name: string;
  file_size: number;
  received_bytes: number;
  chunks_total: number;
  chunks_received: number;
  model_id: string | null;
  created_at_unix_ms: number;
  updated_at_unix_ms: number;
  error: LanTaskError | null;
  result: LanTaskResult | null;
};

export type LanCollectReport = {
  exported: string[];
  failed: [string, string][];
};

export type LanStorageInfo = {
  data_dir: string;
  total_bytes: number;
  free_bytes: number;
};

/** `lan://progress` 事件载荷。 */
export type LanProgressEvent = {
  task_id: string;
  sent: number;
  total: number;
};
