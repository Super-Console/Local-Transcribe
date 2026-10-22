/** Shared frontend types mirroring the Rust command/event payloads. */

export type View = "transcribe" | "history" | "model" | "settings";

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
