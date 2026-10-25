//! On-demand local language-model runtime shared by the desktop and LAN node.
//!
//! The runtime owns one LLM cache slot. The transcription-core ASR cache is
//! explicitly released before a model is loaded, and the caller releases the
//! LLM before starting transcription. Models are never downloaded here; only
//! the three catalogued GGUF files are accepted.

use std::io::Read;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};
use std::time::{Duration, Instant};

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::sampling::LlamaSampler;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sysinfo::{ProcessesToUpdate, System};

const CONTEXT_TOKENS: u32 = 8192;
const MAX_GENERATED_TOKENS: usize = 1024;
const PARTIAL_GENERATED_TOKENS: usize = 512;
const MEMORY_OVERHEAD_BYTES: u64 = 768 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct LlmStatus {
    pub loaded: bool,
    pub busy: bool,
    pub model_id: Option<String>,
    pub loading_model_id: Option<String>,
    pub idle_unload_seconds: u64,
    pub idle_seconds_remaining: Option<u64>,
    pub loading_progress: Option<f32>,
    pub required_memory_bytes: Option<u64>,
    pub asr_model_unloaded: bool,
    pub model_bytes: Option<u64>,
    pub rss_bytes: Option<u64>,
    pub available_memory_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LlmReference {
    pub segment_index: Option<usize>,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LlmModelSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub family: &'static str,
    pub file_name: &'static str,
    pub repository: &'static str,
    pub revision: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
    pub languages: &'static str,
    pub license: &'static str,
}

/// Official, immutable Hugging Face snapshots for the three V2.0 model tiers.
/// Downloads are verified against both the exact byte count and LFS SHA-256.
pub const LLM_MODELS: [LlmModelSpec; 3] = [
    LlmModelSpec {
        id: "qwen3-4b",
        name: "Qwen3-4B",
        family: "Qwen3",
        file_name: "Qwen3-4B-Q4_K_M.gguf",
        repository: "Qwen/Qwen3-4B-GGUF",
        revision: "bc640142c66e1fdd12af0bd68f40445458f3869b",
        bytes: 2_497_280_256,
        sha256: "7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5",
        languages: "zh/multilingual",
        license: "Apache-2.0",
    },
    LlmModelSpec {
        id: "qwen3-8b",
        name: "Qwen3-8B",
        family: "Qwen3",
        file_name: "Qwen3-8B-Q4_K_M.gguf",
        repository: "Qwen/Qwen3-8B-GGUF",
        revision: "7c41481f57cb95916b40956ab2f0b139b296d974",
        bytes: 5_027_783_488,
        sha256: "d98cdcbd03e17ce47681435b5150e34c1417f50b5c0019dd560e4882c5745785",
        languages: "zh/multilingual",
        license: "Apache-2.0",
    },
    LlmModelSpec {
        id: "qwen3-1.7b",
        name: "Qwen3-1.7B",
        family: "Qwen3",
        file_name: "Qwen3-1.7B-Q8_0.gguf",
        repository: "Qwen/Qwen3-1.7B-GGUF",
        revision: "90862c4b9d2787eaed51d12237eafdfe7c5f6077",
        bytes: 1_834_426_016,
        sha256: "061b54daade076b5d3362dac252678d17da8c68f07560be70818cace6590cb1a",
        languages: "multilingual",
        license: "Apache-2.0",
    },
];

#[derive(Default)]
struct RuntimeMeta {
    model_id: Option<String>,
    loading_model_id: Option<String>,
    model_bytes: Option<u64>,
    required_memory_bytes: Option<u64>,
    asr_model_unloaded: bool,
    loading_progress: Option<f32>,
    last_used: Option<Instant>,
}

/// Field order is intentional: Rust drops the model before its backend.
struct LoadedModel {
    model: LlamaModel,
    backend: LlamaBackend,
    _compute_lock: voice_to_text_core::ComputeLockGuard,
    id: String,
    bytes: u64,
}

pub struct LlmRuntime {
    models_dir: PathBuf,
    idle_unload_seconds: AtomicU64,
    operation: Mutex<()>,
    loaded: Mutex<Option<LoadedModel>>,
    meta: Arc<Mutex<RuntimeMeta>>,
    cancelled: Arc<AtomicBool>,
    next_generation_id: AtomicU64,
    generation_id: Mutex<Option<u64>>,
}

struct GenerationGuard<'a> {
    runtime: &'a LlmRuntime,
    id: u64,
}

impl Drop for GenerationGuard<'_> {
    fn drop(&mut self) {
        let mut active = lock(&self.runtime.generation_id);
        if *active == Some(self.id) {
            *active = None;
            self.runtime.cancelled.store(false, Ordering::SeqCst);
        }
    }
}

impl LlmRuntime {
    pub fn new(models_dir: PathBuf, idle_unload_seconds: u64) -> Result<Self, String> {
        if !voice_to_text_core::IDLE_UNLOAD_CHOICES.contains(&idle_unload_seconds) {
            return Err(format!(
                "idle unload must be one of {:?} seconds",
                voice_to_text_core::IDLE_UNLOAD_CHOICES
            ));
        }
        Ok(Self {
            models_dir,
            idle_unload_seconds: AtomicU64::new(idle_unload_seconds),
            operation: Mutex::new(()),
            loaded: Mutex::new(None),
            meta: Arc::new(Mutex::new(RuntimeMeta::default())),
            cancelled: Arc::new(AtomicBool::new(false)),
            next_generation_id: AtomicU64::new(0),
            generation_id: Mutex::new(None),
        })
    }

    pub fn models_dir(&self) -> &Path {
        &self.models_dir
    }

    pub fn set_idle_unload_seconds(&self, seconds: u64) -> Result<(), String> {
        if !voice_to_text_core::IDLE_UNLOAD_CHOICES.contains(&seconds) {
            return Err(format!(
                "idle unload must be one of {:?} seconds",
                voice_to_text_core::IDLE_UNLOAD_CHOICES
            ));
        }
        self.idle_unload_seconds.store(seconds, Ordering::SeqCst);
        Ok(())
    }

    pub fn idle_unload_seconds(&self) -> u64 {
        self.idle_unload_seconds.load(Ordering::SeqCst)
    }

    pub fn status(&self) -> LlmStatus {
        self.unload_if_idle();
        let meta = lock(&self.meta);
        let idle_seconds_remaining = meta.last_used.map(|last_used| {
            self.idle_unload_seconds()
                .saturating_sub(last_used.elapsed().as_secs())
        });
        let (rss_bytes, available_memory_bytes) = memory_snapshot();
        LlmStatus {
            loaded: meta.model_id.is_some(),
            busy: lock(&self.generation_id).is_some() || self.operation.try_lock().is_err(),
            model_id: meta.model_id.clone(),
            loading_model_id: meta.loading_model_id.clone(),
            idle_unload_seconds: self.idle_unload_seconds(),
            idle_seconds_remaining,
            loading_progress: meta.loading_progress,
            required_memory_bytes: meta.required_memory_bytes,
            asr_model_unloaded: meta.asr_model_unloaded,
            model_bytes: meta.model_bytes,
            rss_bytes,
            available_memory_bytes,
        }
    }

    pub fn load(&self, model_id: &str) -> Result<LlmStatus, String> {
        let _operation = lock(&self.operation);
        if lock(&self.generation_id).is_some() {
            return Err("The local language model is busy. Try again shortly.".to_owned());
        }
        self.cancelled.store(false, Ordering::SeqCst);
        self.load_locked(model_id, Arc::new(|_| true))?;
        drop(_operation);
        Ok(self.status_without_idle_check())
    }

    pub fn unload(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        let _operation = lock(&self.operation);
        self.drop_loaded();
        *lock(&self.generation_id) = None;
    }

    /// Stops the current generation. Native token evaluation is synchronous;
    /// generation exits at the next token boundary.
    pub fn cancel_generation(&self) -> bool {
        if lock(&self.generation_id).is_some() {
            self.cancelled.store(true, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    pub fn prepare_generation(&self) -> Result<u64, String> {
        let _operation = match self.operation.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::Poisoned(error)) => error.into_inner(),
            Err(TryLockError::WouldBlock) => {
                return Err("The local language model is busy. Try again shortly.".to_owned());
            }
        };
        let mut active = lock(&self.generation_id);
        if active.is_some() {
            return Err("The local language model is busy. Try again shortly.".to_owned());
        }
        let id = self
            .next_generation_id
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1)
            .max(1);
        self.cancelled.store(false, Ordering::SeqCst);
        *active = Some(id);
        Ok(id)
    }

    pub fn abort_generation(&self, id: u64) {
        let mut active = lock(&self.generation_id);
        if *active == Some(id) {
            *active = None;
            self.cancelled.store(false, Ordering::SeqCst);
        }
    }

    pub fn unload_if_idle(&self) -> bool {
        if lock(&self.generation_id).is_some() {
            return false;
        }
        let meta = lock(&self.meta);
        let should_unload = meta.model_id.is_some()
            && meta.last_used.is_some_and(|last_used| {
                last_used.elapsed() >= Duration::from_secs(self.idle_unload_seconds())
            });
        drop(meta);
        if !should_unload {
            return false;
        }

        let _operation = match self.operation.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::Poisoned(error)) => error.into_inner(),
            Err(TryLockError::WouldBlock) => return false,
        };
        if lock(&self.generation_id).is_some() {
            return false;
        }
        let expired = lock(&self.meta).last_used.is_some_and(|last_used| {
            last_used.elapsed() >= Duration::from_secs(self.idle_unload_seconds())
        });
        if expired {
            self.drop_loaded();
            true
        } else {
            false
        }
    }

    /// Loads the requested catalog model on demand, then emits UTF-8-safe
    /// token pieces. Returning `false` from `on_token` stops generation.
    pub fn generate_stream(
        &self,
        generation_id: u64,
        model_id: &str,
        system_prompt: &str,
        user_prompt: &str,
        on_progress: Arc<dyn Fn(f32) -> bool + Send + Sync>,
        mut on_token: impl FnMut(&str) -> bool,
    ) -> Result<bool, String> {
        let _generation = GenerationGuard {
            runtime: self,
            id: generation_id,
        };
        let _operation = match self.operation.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::Poisoned(error)) => error.into_inner(),
            Err(TryLockError::WouldBlock) => {
                return Err("The local language model is busy. Try again shortly.".to_owned());
            }
        };
        if *lock(&self.generation_id) != Some(generation_id) {
            return Err("This generation request is no longer active.".to_owned());
        }
        if self.cancelled.load(Ordering::SeqCst) {
            return Ok(false);
        }
        self.load_locked(model_id, on_progress)?;
        if self.cancelled.load(Ordering::SeqCst) {
            return Ok(false);
        }

        let loaded = lock(&self.loaded);
        let engine = loaded
            .as_ref()
            .ok_or_else(|| "The local language model is not loaded.".to_owned())?;
        let prompt = format_prompt(system_prompt, user_prompt);
        let result = generate_tokens(engine, &prompt, &self.cancelled, &mut on_token);
        drop(loaded);
        lock(&self.meta).last_used = Some(Instant::now());
        result
    }

    /// Generates a long-transcript answer in map/reduce stages while keeping
    /// every model prompt within 60% of the configured context window. Segment
    /// text streams to `on_partial_token` for a temporary preview; only the
    /// final reduction is streamed to `on_token` as the completed answer.
    #[allow(clippy::too_many_arguments)]
    pub fn generate_segmented_stream(
        &self,
        generation_id: u64,
        model_id: &str,
        system_prompt: &str,
        direct_user_prompt: &str,
        source_text: &str,
        segment_user_prompt: impl Fn(&str) -> String,
        merge_user_prompt: impl Fn(&str) -> String,
        on_progress: Arc<dyn Fn(f32) -> bool + Send + Sync>,
        mut on_stage: impl FnMut(&'static str, usize, usize) -> bool,
        mut on_partial_token: impl FnMut(&str) -> bool,
        mut on_token: impl FnMut(&str) -> bool,
    ) -> Result<bool, String> {
        let _generation = GenerationGuard {
            runtime: self,
            id: generation_id,
        };
        let _operation = match self.operation.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::Poisoned(error)) => error.into_inner(),
            Err(TryLockError::WouldBlock) => {
                return Err("The local language model is busy. Try again shortly.".to_owned());
            }
        };
        if *lock(&self.generation_id) != Some(generation_id) {
            return Err("This generation request is no longer active.".to_owned());
        }
        if self.cancelled.load(Ordering::SeqCst) {
            return Ok(false);
        }
        self.load_locked(model_id, on_progress)?;
        if self.cancelled.load(Ordering::SeqCst) {
            return Ok(false);
        }

        let loaded = lock(&self.loaded);
        let engine = loaded
            .as_ref()
            .ok_or_else(|| "The local language model is not loaded.".to_owned())?;
        let result = (|| {
            let vocab = engine.model.vocab();
            let prompt_budget = CONTEXT_TOKENS as usize * 60 / 100;
            let segment_system_prompt = format!(
            "{system_prompt} This is one segment of a longer transcription. Produce only a concise partial result for this segment; preserve names, numbers, and uncertainty, and do not infer details from unseen segments."
        );
            let merge_system_prompt = format!(
            "{system_prompt} Combine the supplied partial results into one final answer. Use only claims explicitly supported by those partial results; preserve names, numbers, and uncertainty, and add no new facts."
        );
            let segments = segment_text_by_budget(source_text, prompt_budget, |segment| {
                let user = segment_user_prompt(segment);
                vocab
                    .tokenize(
                        format_prompt(&segment_system_prompt, &user).as_bytes(),
                        true,
                        true,
                    )
                    .len()
            })?;

            if segments.len() == 1 {
                let direct_prompt = format_prompt(system_prompt, direct_user_prompt);
                if vocab.tokenize(direct_prompt.as_bytes(), true, true).len() <= prompt_budget {
                    return generate_tokens(engine, &direct_prompt, &self.cancelled, &mut on_token);
                }
                if !on_stage("reading", 1, 1) {
                    return Ok(false);
                }
                let user = segment_user_prompt(&segments[0]);
                let prompt = format_prompt(&segment_system_prompt, &user);
                return generate_tokens(engine, &prompt, &self.cancelled, &mut on_token);
            }

            let mut partials = Vec::with_capacity(segments.len());
            let total_segments = segments.len();
            for (index, segment) in segments.iter().enumerate() {
                if !on_stage("reading", index + 1, total_segments) {
                    return Ok(false);
                }
                if self.cancelled.load(Ordering::SeqCst) {
                    return Ok(false);
                }
                let user = segment_user_prompt(segment);
                let prompt = format_prompt(&segment_system_prompt, &user);
                let mut partial = String::new();
                let completed = generate_tokens_with_limit(
                    engine,
                    &prompt,
                    &self.cancelled,
                    PARTIAL_GENERATED_TOKENS,
                    &mut |token| {
                        partial.push_str(token);
                        on_partial_token(token)
                    },
                )?;
                if !completed {
                    return Ok(false);
                }
                if partial.trim().is_empty() {
                    return Err(format!(
                        "The model returned no result for segment {}.",
                        index + 1
                    ));
                }
                partials.push(partial.trim().to_owned());
            }

            let mut reduction_round = 0;
            while partials.len() > 1 {
                reduction_round += 1;
                let groups = pack_partial_outputs(&partials, prompt_budget, |group| {
                    let user = merge_user_prompt(group);
                    vocab
                        .tokenize(
                            format_prompt(&merge_system_prompt, &user).as_bytes(),
                            true,
                            true,
                        )
                        .len()
                })?;
                if groups.len() >= partials.len() {
                    return Err(
                        "The partial results cannot be reduced within the model context budget."
                            .to_owned(),
                    );
                }

                let mut reduced = Vec::with_capacity(groups.len());
                let total_groups = groups.len();
                for (index, group) in groups.iter().enumerate() {
                    if !on_stage("merging", index + 1, total_groups) {
                        return Ok(false);
                    }
                    if self.cancelled.load(Ordering::SeqCst) {
                        return Ok(false);
                    }
                    let user = merge_user_prompt(group);
                    let prompt = format_prompt(&merge_system_prompt, &user);
                    let is_final = total_groups == 1;
                    let mut partial = String::new();
                    let completed = if is_final {
                        generate_tokens(engine, &prompt, &self.cancelled, &mut on_token)?
                    } else {
                        let mut tokens_since_heartbeat = 0;
                        generate_tokens_with_limit(
                            engine,
                            &prompt,
                            &self.cancelled,
                            PARTIAL_GENERATED_TOKENS,
                            &mut |token| {
                                partial.push_str(token);
                                tokens_since_heartbeat += 1;
                                if tokens_since_heartbeat >= 32 {
                                    tokens_since_heartbeat = 0;
                                    on_stage("merging", index + 1, total_groups)
                                } else {
                                    true
                                }
                            },
                        )?
                    };
                    if !completed {
                        return Ok(false);
                    }
                    if !is_final {
                        if partial.trim().is_empty() {
                            return Err(format!(
                            "The model returned no result while merging round {reduction_round}."
                        ));
                        }
                        reduced.push(partial.trim().to_owned());
                    }
                }
                if groups.len() == 1 {
                    partials.clear();
                } else {
                    partials = reduced;
                }
            }

            Ok(true)
        })();
        drop(loaded);
        lock(&self.meta).last_used = Some(Instant::now());
        result
    }

    fn load_locked(
        &self,
        model_id: &str,
        on_progress: Arc<dyn Fn(f32) -> bool + Send + Sync>,
    ) -> Result<(), String> {
        let spec = model_spec(model_id).ok_or_else(|| {
            "Unknown local language model. Choose a model from the catalog.".to_owned()
        })?;
        let path = self.models_dir.join(spec.file_name);
        if !path.is_file() {
            return Err(format!(
                "The {model_id} model file is not installed. Download it from the model catalog first."
            ));
        }
        let bytes = path
            .metadata()
            .map_err(|error| format!("Cannot inspect the model file: {error}"))?
            .len();
        if bytes != spec.bytes {
            return Err(
                format!(
                    "The {model_id} model file has an unexpected size ({bytes} bytes; expected {}); download it again from the model catalog.",
                    spec.bytes
                ),
            );
        }
        if lock(&self.loaded)
            .as_ref()
            .is_some_and(|loaded| loaded.id == model_id)
        {
            lock(&self.meta).last_used = Some(Instant::now());
            return Ok(());
        }
        {
            let mut meta = lock(&self.meta);
            meta.loading_model_id = Some(model_id.to_owned());
            meta.asr_model_unloaded = false;
            meta.loading_progress = Some(0.0);
        }
        let result = (|| {
            // ASR and LLM are mutually exclusive cache slots in this process.
            let asr_model_unloaded = voice_to_text_core::unload_cached_model()
                .map_err(|error| format!("Cannot release the transcription model: {error}"))?;
            lock(&self.meta).asr_model_unloaded = asr_model_unloaded;

            if lock(&self.loaded)
                .as_ref()
                .is_some_and(|loaded| loaded.id == model_id)
            {
                lock(&self.meta).last_used = Some(Instant::now());
                return Ok(None);
            }

            lock(&self.loaded).take();
            {
                let mut meta = lock(&self.meta);
                meta.model_id = None;
                meta.model_bytes = None;
                meta.required_memory_bytes = None;
                meta.last_used = None;
            }

            let required_memory_bytes = required_memory_bytes(bytes);
            lock(&self.meta).required_memory_bytes = Some(required_memory_bytes);
            // One advisory lock is shared by all ASR/LLM processes using this
            // app-data root. It stays held until the loaded model is dropped.
            let compute_lock =
                voice_to_text_core::acquire_compute_lock_for_models_directory(&self.models_dir)?;
            let (_, available_memory_bytes) = memory_snapshot();
            if let Some(available) =
                available_memory_bytes.filter(|available| *available < required_memory_bytes)
            {
                return Err(format!(
                    "Insufficient memory: this model needs about {} bytes including runtime overhead; {} bytes are currently available. The transcription model cache was unloaded before checking.",
                    required_memory_bytes, available
                ));
            }
            let hash_progress = Arc::clone(&on_progress);
            let hash_meta = Arc::clone(&self.meta);
            verify_model_sha256(&path, spec.sha256, bytes, &self.cancelled, &|progress| {
                let progress = progress * 0.08;
                lock(&hash_meta).loading_progress = Some(progress);
                hash_progress(progress)
            })?;

            let backend = LlamaBackend::init()
                .map_err(|error| format!("Cannot initialize llama.cpp: {error}"))?;
            let cancelled = Arc::clone(&self.cancelled);
            let meta = Arc::clone(&self.meta);
            let params = LlamaModelParams::default().with_use_mmap(true);
            #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
            let params = params.with_n_gpu_layers(0);
            let model_progress = Arc::clone(&on_progress);
            let params = params.with_progress_callback(move |progress| {
                let progress = 0.08 + progress.clamp(0.0, 1.0) * 0.92;
                if let Ok(mut meta) = meta.lock() {
                    meta.loading_progress = Some(progress);
                }
                model_progress(progress) && !cancelled.load(Ordering::SeqCst)
            });
            let model = LlamaModel::load_from_file(&backend, &path, &params).map_err(|error| {
                if self.cancelled.load(Ordering::SeqCst) {
                    "Model loading was cancelled.".to_owned()
                } else {
                    format!("llama.cpp could not load the GGUF model: {error}")
                }
            })?;
            Ok::<_, String>(Some(LoadedModel {
                model,
                backend,
                _compute_lock: compute_lock,
                id: model_id.to_owned(),
                bytes,
            }))
        })();

        let mut meta = lock(&self.meta);
        meta.loading_model_id = None;
        match result {
            Ok(Some(loaded)) => {
                meta.model_id = Some(loaded.id.clone());
                meta.model_bytes = Some(loaded.bytes);
                meta.loading_progress = None;
                meta.last_used = Some(Instant::now());
                *lock(&self.loaded) = Some(loaded);
                Ok(())
            }
            Ok(None) => {
                meta.loading_progress = None;
                Ok(())
            }
            Err(error) => {
                if lock(&self.loaded).is_none() {
                    meta.model_id = None;
                    meta.model_bytes = None;
                    meta.last_used = None;
                }
                meta.loading_progress = None;
                Err(error)
            }
        }
    }

    fn drop_loaded(&self) {
        lock(&self.loaded).take();
        let mut meta = lock(&self.meta);
        meta.model_id = None;
        meta.loading_model_id = None;
        meta.model_bytes = None;
        meta.required_memory_bytes = None;
        meta.asr_model_unloaded = false;
        meta.loading_progress = None;
        meta.last_used = None;
        self.cancelled.store(false, Ordering::SeqCst);
    }

    fn status_without_idle_check(&self) -> LlmStatus {
        let meta = lock(&self.meta);
        let idle_seconds_remaining = meta.last_used.map(|last_used| {
            self.idle_unload_seconds()
                .saturating_sub(last_used.elapsed().as_secs())
        });
        let (rss_bytes, available_memory_bytes) = memory_snapshot();
        LlmStatus {
            loaded: meta.model_id.is_some(),
            busy: lock(&self.generation_id).is_some() || self.operation.try_lock().is_err(),
            model_id: meta.model_id.clone(),
            loading_model_id: meta.loading_model_id.clone(),
            idle_unload_seconds: self.idle_unload_seconds(),
            idle_seconds_remaining,
            loading_progress: meta.loading_progress,
            required_memory_bytes: meta.required_memory_bytes,
            asr_model_unloaded: meta.asr_model_unloaded,
            model_bytes: meta.model_bytes,
            rss_bytes,
            available_memory_bytes,
        }
    }
}

pub fn required_memory_bytes(model_bytes: u64) -> u64 {
    model_bytes
        .saturating_mul(5)
        .saturating_add(3)
        .saturating_div(4)
        .saturating_add(MEMORY_OVERHEAD_BYTES)
}

fn verify_model_sha256(
    path: &Path,
    expected: &str,
    total_bytes: u64,
    cancelled: &AtomicBool,
    on_progress: &dyn Fn(f32) -> bool,
) -> Result<(), String> {
    let mut file = std::fs::File::open(path).map_err(|error| {
        format!("Cannot open the language model for integrity checking: {error}")
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 4 * 1024 * 1024];
    let mut bytes_read = 0u64;
    loop {
        if cancelled.load(Ordering::SeqCst) {
            return Err("Model loading was cancelled.".to_owned());
        }
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("Cannot verify the language model: {error}"))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        bytes_read = bytes_read.saturating_add(count as u64);
        if !on_progress((bytes_read as f64 / total_bytes.max(1) as f64).clamp(0.0, 1.0) as f32) {
            return Err("Model loading was cancelled.".to_owned());
        }
    }
    let actual = format!("{:x}", hasher.finalize());
    if actual != expected {
        return Err(
            "The model file SHA-256 does not match the catalog; download it again.".to_owned(),
        );
    }
    Ok(())
}

pub fn model_spec(model_id: &str) -> Option<&'static LlmModelSpec> {
    LLM_MODELS.iter().find(|spec| spec.id == model_id)
}

#[cfg(test)]
fn model_file_name(model_id: &str) -> Option<&'static str> {
    model_spec(model_id).map(|spec| spec.file_name)
}

pub fn split_reference_segments(text: &str) -> Vec<String> {
    text.split(['\n', '。', '！', '？', '!', '?', '.'])
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(str::to_owned)
        .collect()
}

pub fn select_references(segments: &[String], question: &str) -> Vec<LlmReference> {
    let terms = query_terms(question)
        .into_iter()
        .map(|term| term.to_lowercase())
        .collect::<Vec<_>>();
    let mut ranked = segments
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let normalized_text = text.to_lowercase();
            let score = terms
                .iter()
                .filter(|term| normalized_text.contains(term.as_str()))
                .count();
            (score, index, text)
        })
        .filter(|(score, _, _)| *score > 0)
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    ranked
        .into_iter()
        .take(3)
        .map(|(_, index, text)| LlmReference {
            segment_index: Some(index),
            text: text.chars().take(240).collect(),
        })
        .collect()
}

fn query_terms(question: &str) -> Vec<String> {
    fn append_cjk_bigrams(text: &mut String, terms: &mut Vec<String>) {
        let characters = text.chars().collect::<Vec<_>>();
        for pair in characters.windows(2) {
            let term = pair.iter().collect::<String>();
            if !terms.contains(&term) {
                terms.push(term);
            }
        }
        text.clear();
    }

    let mut terms = Vec::new();
    let mut cjk = String::new();
    let mut word = String::new();
    let flush_word = |word: &mut String, terms: &mut Vec<String>| {
        if word.chars().count() >= 2 && !terms.contains(word) {
            terms.push(std::mem::take(word));
        } else {
            word.clear();
        }
    };
    for character in question.chars() {
        if is_cjk(character) {
            flush_word(&mut word, &mut terms);
            cjk.push(character);
        } else {
            append_cjk_bigrams(&mut cjk, &mut terms);
            if character.is_alphanumeric() {
                word.push(character);
            } else {
                flush_word(&mut word, &mut terms);
            }
        }
    }
    append_cjk_bigrams(&mut cjk, &mut terms);
    flush_word(&mut word, &mut terms);
    terms
}

fn is_cjk(character: char) -> bool {
    matches!(
        character as u32,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF
    )
}

#[derive(Default)]
struct ThinkTagFilter {
    pending: String,
    suppress_until_end: bool,
}

impl ThinkTagFilter {
    fn push(&mut self, chunk: &str) -> String {
        const OPEN: &str = "<think>";
        const CLOSE: &str = "</think>";

        self.pending.push_str(chunk);
        let mut visible = String::new();
        loop {
            if self.suppress_until_end {
                if let Some(end) = self.pending.find(CLOSE) {
                    self.pending.drain(..end + CLOSE.len());
                    self.suppress_until_end = false;
                    continue;
                }
                let keep = longest_marker_prefix_suffix(&self.pending, &[CLOSE]);
                self.pending.drain(..self.pending.len() - keep);
                break;
            }

            let open = self.pending.find(OPEN);
            let close = self.pending.find(CLOSE);
            match (open, close) {
                (Some(open), Some(close)) if open <= close => {
                    visible.push_str(&self.pending[..open]);
                    self.pending.drain(..open + OPEN.len());
                    self.suppress_until_end = true;
                }
                (Some(open), None) => {
                    visible.push_str(&self.pending[..open]);
                    self.pending.drain(..open + OPEN.len());
                    self.suppress_until_end = true;
                }
                (_, Some(close)) => {
                    visible.push_str(&self.pending[..close]);
                    self.pending.drain(..close + CLOSE.len());
                }
                (None, None) => {
                    let keep = longest_marker_prefix_suffix(&self.pending, &[OPEN, CLOSE]);
                    let safe = self.pending.len() - keep;
                    visible.push_str(&self.pending[..safe]);
                    self.pending.drain(..safe);
                    break;
                }
            }
        }
        visible
    }

    fn finish(&mut self) -> String {
        // Pending bytes are either a partial control tag or part of an
        // unterminated thinking block; neither should leak into the answer.
        self.pending.clear();
        String::new()
    }
}

fn longest_marker_prefix_suffix(text: &str, markers: &[&str]) -> usize {
    markers
        .iter()
        .map(|marker| {
            (1..=text.len().min(marker.len().saturating_sub(1)))
                .rev()
                .find(|length| text.as_bytes().ends_with(&marker.as_bytes()[..*length]))
                .unwrap_or(0)
        })
        .max()
        .unwrap_or(0)
}

fn segment_text_by_budget(
    text: &str,
    max_tokens: usize,
    mut token_count: impl FnMut(&str) -> usize,
) -> Result<Vec<String>, String> {
    if max_tokens == 0 {
        return Err("The model context leaves no room for a transcript segment.".to_owned());
    }
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut units = Vec::new();
    for line in normalized.split('\n') {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        for (index, sentence) in split_sentences(line).into_iter().enumerate() {
            units.push((sentence, index == 0));
        }
    }
    if units.is_empty() {
        return Err("The transcription is empty after removing whitespace.".to_owned());
    }

    let mut segments = Vec::new();
    let mut current = String::new();
    for (unit, starts_line) in units {
        let separator = if current.is_empty() {
            ""
        } else if starts_line {
            "\n\n"
        } else if unit
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii())
        {
            " "
        } else {
            ""
        };
        let candidate = format!("{current}{separator}{unit}");
        if token_count(&candidate) <= max_tokens {
            current = candidate;
            continue;
        }

        if !current.is_empty() {
            segments.push(std::mem::take(&mut current));
        }
        if token_count(&unit) > max_tokens {
            return Err(format!(
                "A single sentence exceeds the {max_tokens}-token segment budget; it was not truncated."
            ));
        }
        current = unit;
    }
    if !current.is_empty() {
        segments.push(current);
    }
    Ok(segments)
}

fn split_sentences(text: &str) -> Vec<String> {
    const CLOSING_PUNCTUATION: &str = "\"'”’」』】）》〕〉»)]}";
    const ABBREVIATIONS: &[&str] = &[
        "mr.", "mrs.", "ms.", "dr.", "prof.", "sr.", "jr.", "st.", "e.g.", "i.e.", "etc.",
    ];

    let chars = text.char_indices().collect::<Vec<_>>();
    let mut sentences = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < chars.len() {
        let (offset, character) = chars[index];
        let mut after = index + 1;
        while after < chars.len() && CLOSING_PUNCTUATION.contains(chars[after].1) {
            after += 1;
        }
        let next = chars.get(after).map(|(_, value)| *value);
        let follows_space_or_end = next.is_none_or(char::is_whitespace);
        let previous = index
            .checked_sub(1)
            .and_then(|previous| chars.get(previous));
        let is_decimal_point = character == '.'
            && previous.is_some_and(|(_, value)| value.is_ascii_digit())
            && next.is_some_and(|value| value.is_ascii_digit());
        let is_abbreviation = character == '.'
            && ABBREVIATIONS.iter().any(|abbreviation| {
                let end = offset + character.len_utf8();
                let start = end.saturating_sub(abbreviation.len());
                let starts_word = start == 0
                    || text
                        .get(..start)
                        .and_then(|prefix| prefix.chars().next_back())
                        .is_some_and(|character| character.is_whitespace());
                starts_word
                    && text
                        .get(start..end)
                        .is_some_and(|suffix| suffix.eq_ignore_ascii_case(abbreviation))
            });
        let sentence_end = matches!(character, '。' | '！' | '？')
            || (matches!(character, '!' | '?') && follows_space_or_end)
            || (character == '.' && follows_space_or_end && !is_decimal_point && !is_abbreviation);

        if sentence_end {
            let end = if after == 0 {
                offset + character.len_utf8()
            } else {
                chars[after - 1].0 + chars[after - 1].1.len_utf8()
            };
            let sentence = text[start..end].trim();
            if !sentence.is_empty() {
                sentences.push(sentence.to_owned());
            }
            start = end;
            while after < chars.len() && chars[after].1.is_whitespace() {
                start = chars[after].0 + chars[after].1.len_utf8();
                after += 1;
            }
            index = after;
        } else {
            index += 1;
        }
    }
    let trailing = text[start..].trim();
    if !trailing.is_empty() {
        sentences.push(trailing.to_owned());
    }
    sentences
}

fn pack_partial_outputs(
    outputs: &[String],
    max_tokens: usize,
    mut token_count: impl FnMut(&str) -> usize,
) -> Result<Vec<String>, String> {
    let mut groups = Vec::new();
    let mut current = String::new();
    for output in outputs {
        let candidate = if current.is_empty() {
            output.clone()
        } else {
            format!("{current}\n\n{output}")
        };
        if token_count(&candidate) <= max_tokens {
            current = candidate;
            continue;
        }
        if current.is_empty() {
            return Err(format!(
                "A partial result exceeds the {max_tokens}-token merge budget."
            ));
        }
        groups.push(std::mem::take(&mut current));
        if token_count(output) > max_tokens {
            return Err(format!(
                "A partial result exceeds the {max_tokens}-token merge budget."
            ));
        }
        current = output.clone();
    }
    if !current.is_empty() {
        groups.push(current);
    }
    Ok(groups)
}

fn emit_filtered_token(
    filter: &mut ThinkTagFilter,
    text: &str,
    on_token: &mut impl FnMut(&str) -> bool,
) -> bool {
    let visible = filter.push(text);
    visible.is_empty() || on_token(&visible)
}

fn format_prompt(system_prompt: &str, user_prompt: &str) -> String {
    format!(
        "<|im_start|>system\n{system_prompt}\n/no_think<|im_end|>\n<|im_start|>user\n{user_prompt}<|im_end|>\n<|im_start|>assistant\n"
    )
}

fn generate_tokens(
    engine: &LoadedModel,
    prompt: &str,
    cancelled: &AtomicBool,
    on_token: &mut impl FnMut(&str) -> bool,
) -> Result<bool, String> {
    generate_tokens_with_limit(engine, prompt, cancelled, MAX_GENERATED_TOKENS, on_token)
}

fn generate_tokens_with_limit(
    engine: &LoadedModel,
    prompt: &str,
    cancelled: &AtomicBool,
    generated_token_limit: usize,
    on_token: &mut impl FnMut(&str) -> bool,
) -> Result<bool, String> {
    let mut context = engine
        .model
        .new_context(&engine.backend, llm_context_params())
        .map_err(|error| format!("Cannot create the model context: {error}"))?;
    let vocab = engine.model.vocab();
    let tokens = vocab.tokenize(prompt.as_bytes(), true, true);
    if tokens.is_empty() {
        return Err("The prompt is empty after tokenization.".to_owned());
    }
    if tokens.len() + 1 >= context.n_ctx() as usize {
        return Err(format!(
            "The prompt needs {} tokens, exceeding the {}-token model context.",
            tokens.len(),
            context.n_ctx()
        ));
    }

    let mut batch = LlamaBatch::new(tokens.len(), 1);
    for (index, token) in tokens.iter().enumerate() {
        batch
            .add(
                *token,
                i32::try_from(index).map_err(|_| "Prompt is too long.".to_owned())?,
                &[0],
                index + 1 == tokens.len(),
            )
            .map_err(|error| format!("Cannot prepare the model prompt: {error}"))?;
    }
    context
        .decode(&mut batch)
        .map_err(|error| format!("Cannot evaluate the model prompt: {error}"))?;
    if cancelled.load(Ordering::SeqCst) {
        return Ok(false);
    }

    let mut sampler = LlamaSampler::chain_simple([
        LlamaSampler::temp(0.2),
        LlamaSampler::top_p(0.9, 1),
        LlamaSampler::dist(42),
    ]);
    let first_position = i32::try_from(tokens.len()).map_err(|_| "Prompt is too long.")?;
    let max_new_tokens =
        generated_token_limit.min(context.n_ctx().saturating_sub(tokens.len() as u32) as usize);
    let mut pending = Vec::new();
    let mut think_filter = ThinkTagFilter::default();

    for step in 0..max_new_tokens {
        if cancelled.load(Ordering::SeqCst) {
            return Ok(false);
        }
        let position = first_position
            .checked_add(i32::try_from(step).map_err(|_| "Prompt is too long.")?)
            .ok_or("Prompt is too long.")?;
        let token = sampler.sample(&context, -1);
        if vocab.is_eog(token) {
            break;
        }
        pending.extend(vocab.token_to_piece(token, false, None));
        if !emit_utf8_prefix(&mut pending, &mut |text| {
            emit_filtered_token(&mut think_filter, text, on_token)
        }) {
            return Ok(false);
        }
        batch.clear();
        batch
            .add(token, position, &[0], true)
            .map_err(|error| format!("Cannot prepare the next model token: {error}"))?;
        context
            .decode(&mut batch)
            .map_err(|error| format!("Model token evaluation failed: {error}"))?;
    }

    if !pending.is_empty() {
        let trailing = String::from_utf8_lossy(&pending);
        if !trailing.is_empty() && !emit_filtered_token(&mut think_filter, &trailing, on_token) {
            return Ok(false);
        }
    }
    let trailing_visible = think_filter.finish();
    if !trailing_visible.is_empty() && !on_token(&trailing_visible) {
        return Ok(false);
    }
    Ok(!cancelled.load(Ordering::SeqCst))
}

fn llm_context_params() -> LlamaContextParams {
    LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(CONTEXT_TOKENS))
        .with_n_batch(CONTEXT_TOKENS)
        .with_n_threads(available_threads())
        .with_n_threads_batch(available_threads())
}

fn emit_utf8_prefix(pending: &mut Vec<u8>, on_token: &mut impl FnMut(&str) -> bool) -> bool {
    match std::str::from_utf8(pending) {
        Ok(text) => {
            let result = text.is_empty() || on_token(text);
            pending.clear();
            result
        }
        Err(error) => {
            let valid = error.valid_up_to();
            if valid > 0 {
                let text = std::str::from_utf8(&pending[..valid])
                    .expect("valid_up_to always points to a valid UTF-8 prefix");
                if !on_token(text) {
                    return false;
                }
                pending.drain(..valid);
            }
            let had_invalid_sequence = error.error_len().is_some();
            if had_invalid_sequence {
                let text = String::from_utf8_lossy(pending);
                if !text.is_empty() && !on_token(&text) {
                    return false;
                }
                pending.clear();
            }
            true
        }
    }
}

fn available_threads() -> i32 {
    std::thread::available_parallelism().map_or(2, |count| count.get().clamp(1, 8) as i32)
}

fn memory_snapshot() -> (Option<u64>, Option<u64>) {
    let pid = sysinfo::get_current_pid().ok();
    let mut system = System::new();
    system.refresh_memory();
    let available = Some(system.available_memory());
    let rss = pid.and_then(|pid| {
        system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
        system.process(pid).map(|process| process.memory())
    });
    (rss, available)
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::{
        llm_context_params, model_file_name, model_spec, pack_partial_outputs,
        required_memory_bytes, segment_text_by_budget, split_sentences, verify_model_sha256,
        LlmRuntime, ThinkTagFilter, CONTEXT_TOKENS, LLM_MODELS,
    };
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn filter_chunks(chunks: &[&str]) -> String {
        let mut filter = ThinkTagFilter::default();
        let mut visible = chunks
            .iter()
            .map(|chunk| filter.push(chunk))
            .collect::<String>();
        visible.push_str(&filter.finish());
        visible
    }

    #[test]
    fn llama_decode_batch_capacity_covers_the_segment_prompt_budget() {
        let max_segment_prompt_tokens = CONTEXT_TOKENS as usize * 60 / 100;
        assert!(
            llm_context_params().n_batch() as usize >= max_segment_prompt_tokens,
            "llama_decode n_batch must cover the {max_segment_prompt_tokens}-token segment budget"
        );
    }

    #[test]
    fn think_filter_hides_reasoning_and_preserves_the_answer() {
        assert_eq!(
            filter_chunks(&[
                "visible ",
                "<think>",
                "private reasoning",
                "</think>",
                "answer"
            ]),
            "visible answer"
        );
    }

    #[test]
    fn think_filter_handles_tags_split_across_stream_chunks() {
        assert_eq!(
            filter_chunks(&["<thi", "nk>private</", "think", ">Answer"]),
            "Answer"
        );
    }

    #[test]
    fn think_filter_keeps_ordinary_text_and_discards_unclosed_thinking() {
        assert_eq!(filter_chunks(&["A <thinking> word"]), "A <thinking> word");
        assert_eq!(filter_chunks(&["<think>private", " still private"]), "");
    }

    #[test]
    fn transcript_segmentation_prefers_paragraph_then_sentence_boundaries() {
        let source = "甲乙丙丁。戊己庚辛。\n壬癸子丑。寅卯辰巳。";
        let segments = segment_text_by_budget(source, 10, |value| {
            value
                .chars()
                .filter(|character| !character.is_whitespace())
                .count()
        })
        .unwrap();
        assert_eq!(segments, ["甲乙丙丁。戊己庚辛。", "壬癸子丑。寅卯辰巳。"]);
    }

    #[test]
    fn transcript_segmentation_never_splits_a_sentence_and_fails_if_one_is_oversized() {
        let segments = segment_text_by_budget("甲乙丙丁。戊己庚辛。", 5, |value| {
            value.chars().count()
        })
        .unwrap();
        assert_eq!(segments, ["甲乙丙丁。", "戊己庚辛。"]);
        assert!(
            segment_text_by_budget("一个没有可用句界的超长句子", 4, |value| {
                value.chars().count()
            })
            .is_err()
        );
    }

    #[test]
    fn sentence_scanner_keeps_decimals_abbreviations_and_closing_quotes_intact() {
        assert_eq!(
            split_sentences("Dr. Chen measured 3.14 cm. ‘Next sentence!’ Final."),
            ["Dr. Chen measured 3.14 cm.", "‘Next sentence!’", "Final."]
        );
    }

    #[test]
    fn merge_packing_keeps_partial_outputs_atomic_and_within_budget() {
        let outputs = ["one", "two", "a", "b"].map(str::to_owned);
        let groups = pack_partial_outputs(&outputs, 8, str::len).unwrap();
        assert_eq!(groups, ["one\n\ntwo", "a\n\nb"]);
        assert!(pack_partial_outputs(&["too large".to_owned()], 3, str::len).is_err());
    }

    #[test]
    fn only_catalogued_gguf_files_can_be_loaded() {
        assert_eq!(model_file_name("qwen3-4b"), Some("Qwen3-4B-Q4_K_M.gguf"));
        assert_eq!(model_file_name("qwen3-8b"), Some("Qwen3-8B-Q4_K_M.gguf"));
        assert_eq!(model_file_name("qwen3-1.7b"), Some("Qwen3-1.7B-Q8_0.gguf"));
        assert_eq!(model_file_name("../../secret"), None);
        assert_eq!(model_file_name("arbitrary.gguf"), None);
        assert_eq!(LLM_MODELS.len(), 3);
        assert_eq!(model_spec("qwen3-8b").unwrap().bytes, 5_027_783_488);
        assert_eq!(model_spec("qwen3-1.7b").unwrap().license, "Apache-2.0");
    }

    #[test]
    fn memory_estimate_includes_model_mapping_and_context_overhead() {
        let model_size = 2_500_000_000;
        assert_eq!(required_memory_bytes(model_size), 3_930_306_368);
        assert!(required_memory_bytes(5_000_000_000) > 7_000_000_000);
    }

    #[test]
    fn catalog_file_integrity_check_detects_corruption() {
        let path =
            std::env::temp_dir().join(format!("llm-integrity-{}.gguf", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"verified model bytes").unwrap();
        let expected = "03cfa25d83f5eaa1faac98ed6ceaaf0e7afe3c273a1e1502c2714ebe10b8263e";
        let progress = |_| true;
        assert!(
            verify_model_sha256(&path, expected, 20, &AtomicBool::new(false), &progress).is_ok()
        );
        assert!(verify_model_sha256(
            &path,
            &"0".repeat(64),
            20,
            &AtomicBool::new(false),
            &progress
        )
        .is_err());
    }

    #[test]
    fn status_reports_busy_and_idle_timeout_updates_are_validated() {
        let runtime = LlmRuntime::new(PathBuf::new(), 120).unwrap();
        let operation = runtime.operation.lock().unwrap();
        assert!(runtime.status_without_idle_check().busy);
        drop(operation);
        assert!(runtime.set_idle_unload_seconds(600).is_ok());
        assert_eq!(runtime.status_without_idle_check().idle_unload_seconds, 600);
        assert!(runtime.set_idle_unload_seconds(0).is_err());
        let generation_id = runtime.prepare_generation().unwrap();
        assert!(runtime.status_without_idle_check().busy);
        assert!(runtime.prepare_generation().is_err());
        assert!(runtime.cancel_generation());
        assert!(runtime.cancelled.load(Ordering::SeqCst));
        runtime.abort_generation(generation_id);
        assert!(!runtime.cancelled.load(Ordering::SeqCst));
        assert!(!runtime.cancel_generation());
        assert!(!runtime.status_without_idle_check().busy);
    }
}
