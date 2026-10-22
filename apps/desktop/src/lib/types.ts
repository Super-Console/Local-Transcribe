/** Shared frontend types mirroring the Rust command/event payloads. */

export type View = "home" | "model" | "settings";

export type InputFile = { path: string; fileName: string; sizeBytes: number };

export type ModelSummary = { id: string; displayName: string; family: string };

export type Segment = { startMs: number; endMs: number; text: string };

export type LibraryModelSummary = {
  id: string;
  displayName: string;
  family: string;
  bytes: number;
  selected: boolean;
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
