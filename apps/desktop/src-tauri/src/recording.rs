//! 本地录音：任意条音轨，每条可以是输入设备（麦克风）或指定应用（Core Audio Process Tap）。
#![allow(unsafe_op_in_unsafe_fn)]
//!
//! 每条轨道各自写一个 WAV 文件，录完分别进入转写管线。

use std::{
    fs::{self, File},
    io::BufWriter,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Instant,
};

use serde::Serialize;
use tauri::AppHandle;

#[derive(Default)]
pub struct RecordingState {
    session: Mutex<Option<Session>>,
}

struct Session {
    stop: Arc<AtomicBool>,
    started: Instant,
    tracks: Vec<TrackHandle>,
}

/// 一条录音轨道：输入设备（麦克风）或指定应用。
struct TrackHandle {
    label: String,
    path: PathBuf,
    level: Arc<AtomicU32>,
    join: Option<JoinHandle<()>>,
    error: Arc<Mutex<Option<String>>>,
}

/// 前端提交的轨道描述：kind = "input" | "app"，target = 设备名或 PID。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackSpec {
    pub kind: String,
    pub target: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordedTrack {
    pub path: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordedTracks {
    pub tracks: Vec<RecordedTrack>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackLevel {
    pub label: String,
    pub level: f32,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingStatus {
    pub running: bool,
    pub elapsed_ms: u64,
    pub tracks: Vec<TrackLevel>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioApp {
    pub pid: i32,
    pub name: String,
}

fn store_level(slot: &AtomicU32, peak: f32) {
    slot.store(peak.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
}

fn load_level(slot: &AtomicU32) -> f32 {
    f32::from_bits(slot.load(Ordering::Relaxed))
}

fn empty_error() -> Arc<Mutex<Option<String>>> {
    Arc::new(Mutex::new(None))
}

// ---------------------------------------------------------------------------
// 输入设备（麦克风）
// ---------------------------------------------------------------------------

fn spawn_input(device_name: Option<String>, label: &str, path: PathBuf, stop: Arc<AtomicBool>) -> TrackHandle {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let level = Arc::new(AtomicU32::new(0.0f32.to_bits()));
    let error = empty_error();
    let label_owned = label.to_owned();
    let handle_path = path.clone();

    let mut handle = TrackHandle {
        label: label_owned,
        path: handle_path,
        level: Arc::clone(&level),
        join: None,
        error: Arc::clone(&error),
    };

    let host = cpal::default_host();
    let device = match device_name.as_deref() {
        Some(name) if !name.is_empty() => host
            .input_devices()
            .ok()
            .and_then(|mut devices| {
                devices.find(|device| device.name().map(|value| value == name).unwrap_or(false))
            }),
        _ => host.default_input_device(),
    };
    let Some(device) = device else {
        if let Ok(mut slot) = error.lock() {
            *slot = Some("找不到该输入设备。".to_owned());
        }
        return handle;
    };
    let Ok(config) = device.default_input_config() else {
        if let Ok(mut slot) = error.lock() {
            *slot = Some("无法读取麦克风配置（检查麦克风权限）。".to_owned());
        }
        return handle;
    };
    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let sample_format = config.sample_format();

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let writer = match hound::WavWriter::create(
        &path,
        hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    ) {
        Ok(writer) => writer,
        Err(issue) => {
            if let Ok(mut slot) = error.lock() {
                *slot = Some(format!("无法写入录音文件：{issue}"));
            }
            return handle;
        }
    };

    let join = thread::Builder::new()
        .name("recording-input".to_owned())
        .spawn(move || {
            let writer = Arc::new(Mutex::new(Some(writer)));
            let sink = Arc::clone(&writer);
            let lvl = Arc::clone(&level);
            let mut peak: f32 = 0.0;
            let mut peak_frames: usize = 0;

            let stream = device.build_input_stream(
                &config.config(),
                move |data: &[f32], _| {
                    let Ok(mut guard) = sink.lock() else { return };
                    let Some(wav) = guard.as_mut() else { return };
                    for frame in data.chunks(channels.max(1)) {
                        let sum: f32 = frame.iter().copied().sum();
                        let mono = sum / channels.max(1) as f32;
                        peak = peak.max(mono.abs());
                        let _ = wav.write_sample((mono * i16::MAX as f32) as i16);
                    }
                    peak_frames += data.len() / channels.max(1);
                    if peak_frames >= (sample_rate as usize / 20).max(1) {
                        store_level(&lvl, peak);
                        peak = 0.0;
                        peak_frames = 0;
                    }
                },
                |issue| eprintln!("input stream error: {issue}"),
                None,
            );

            match stream {
                Ok(stream) => {
                    let _ = stream.play();
                    while !stop.load(Ordering::Relaxed) {
                        thread::sleep(std::time::Duration::from_millis(50));
                    }
                    drop(stream);
                    if let Ok(mut guard) = writer.lock()
                        && let Some(wav) = guard.take()
                    {
                        let _ = wav.finalize();
                    }
                }
                Err(issue) => {
                    if let Ok(mut slot) = error.lock() {
                        *slot = Some(format!("无法启动麦克风：{issue}"));
                    }
                }
            }
        })
        .ok();

    if let Some(join) = join {
        handle.join = Some(join);
    }
    let _ = sample_format;
    handle
}

// ---------------------------------------------------------------------------
// 应用音频（Core Audio Process Tap）
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn spawn_app(pid: i32, label: &str, path: PathBuf, stop: Arc<AtomicBool>) -> TrackHandle {
    let level = Arc::new(AtomicU32::new(0.0f32.to_bits()));
    let error = empty_error();
    let mut handle = TrackHandle {
        label: label.to_owned(),
        path: path.clone(),
        level: Arc::clone(&level),
        join: None,
        error: Arc::clone(&error),
    };

    let thread_path = path;
    let thread_level = Arc::clone(&level);
    let thread_stop = Arc::clone(&stop);
    let thread_error = Arc::clone(&error);
    match thread::Builder::new()
        .name("recording-app".to_owned())
        .spawn(move || run_app_tap(pid, thread_path, thread_level, thread_stop, thread_error))
    {
        Ok(join) => handle.join = Some(join),
        Err(issue) => {
            if let Ok(mut slot) = error.lock() {
                *slot = Some(issue.to_string());
            }
        }
    }
    handle
}

#[cfg(not(target_os = "macos"))]
fn spawn_app(_pid: i32, label: &str, path: PathBuf, _stop: Arc<AtomicBool>) -> TrackHandle {
    let error = empty_error();
    if let Ok(mut slot) = error.lock() {
        *slot = Some("录制应用音频目前仅支持 macOS。".to_owned());
    }
    TrackHandle {
        label: label.to_owned(),
        path,
        level: Arc::new(AtomicU32::new(0.0f32.to_bits())),
        join: None,
        error,
    }
}

#[cfg(target_os = "macos")]
fn run_app_tap(
    pid: i32,
    path: PathBuf,
    level: Arc<AtomicU32>,
    stop: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
) {
    if let Err(message) = unsafe { start_tap_session(pid, &path, &level, &stop) }
        && let Ok(mut slot) = error.lock()
    {
        *slot = Some(message);
    }
}

#[cfg(target_os = "macos")]
unsafe fn start_tap_session(
    pid: i32,
    path: &Path,
    level: &Arc<AtomicU32>,
    stop: &Arc<AtomicBool>,
) -> Result<(), String> {
    use objc2::AllocAnyThread;
    use objc2_core_audio::{
        AudioDeviceCreateIOProcID, AudioDeviceDestroyIOProcID, AudioDeviceStart, AudioDeviceStop,
        AudioHardwareCreateAggregateDevice, AudioHardwareCreateProcessTap,
        AudioHardwareDestroyAggregateDevice, AudioHardwareDestroyProcessTap,
        AudioObjectGetPropertyData, AudioObjectPropertyAddress, CATapDescription,
        kAudioObjectPropertyElementMain, kAudioObjectPropertyScopeGlobal,
    };
    use objc2_core_audio_types::{AudioBuffer, AudioBufferList, AudioTimeStamp};
    use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString};

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|issue| issue.to_string())?;
    }

    // 1) PID → AudioObjectID
    let mut obj_id: u32 = 0;
    let mut address = AudioObjectPropertyAddress {
        mSelector: u32::from_be_bytes(*b"id2p"),
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    };
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = AudioObjectGetPropertyData(
        1,
        std::ptr::NonNull::new(&mut address).unwrap(),
        std::mem::size_of::<i32>() as u32,
        &pid as *const i32 as *const std::ffi::c_void,
        std::ptr::NonNull::new(&mut size).unwrap(),
        std::ptr::NonNull::new(&mut obj_id as *mut u32 as *mut std::ffi::c_void).unwrap(),
    );
    if status != 0 || obj_id == 0 {
        return Err(format!("无法定位该应用的音频（错误 {status}）"));
    }

    // 2) 描述该进程的音频
    let numbers = NSArray::from_retained_slice(&[NSNumber::new_u32(obj_id)]);
    let description =
        CATapDescription::initStereoMixdownOfProcesses(CATapDescription::alloc(), &numbers);
    description.setName(&NSString::from_str("Local Transcribe recording"));
    description.setPrivate(true);

    // 3) 创建 tap
    let mut tap_id: u32 = 0;
    let status = AudioHardwareCreateProcessTap(Some(&description), &mut tap_id as *mut u32);
    if status != 0 {
        return Err(format!(
            "无法创建系统音频捕获（错误 {status}，需要「系统音频录制」权限）"
        ));
    }

    // 4) 聚合设备
    let uid = description.UUID().UUIDString();
    let tap_entry = NSDictionary::from_slices(
        &[&*NSString::from_str("uid")],
        &[&*uid as &objc2::runtime::AnyObject],
    );
    let sub_devices = NSArray::from_retained_slice(&[tap_entry]);
    let dict = NSDictionary::from_slices(
        &[
            &*NSString::from_str("uid"),
            &*NSString::from_str("name"),
            &*NSString::from_str("subdevices"),
            &*NSString::from_str("private"),
        ],
        &[
            &*NSString::from_str(&format!("lt-tap-{tap_id}")) as &objc2::runtime::AnyObject,
            &*NSString::from_str("Local Transcribe tap"),
            &*sub_devices as &objc2::runtime::AnyObject,
            &*NSNumber::new_bool(true) as &objc2::runtime::AnyObject,
        ],
    );
    let cf_dict: &objc2_core_foundation::CFDictionary =
        &*(objc2::rc::Retained::as_ptr(&dict) as *const objc2_core_foundation::CFDictionary);

    let mut aggregate_id: u32 = 0;
    let status = AudioHardwareCreateAggregateDevice(
        cf_dict,
        std::ptr::NonNull::new(&mut aggregate_id as *mut u32).unwrap(),
    );
    if status != 0 {
        AudioHardwareDestroyProcessTap(tap_id);
        return Err(format!("无法创建录音聚合设备（错误 {status}）"));
    }

    struct Context {
        writer: Mutex<Option<hound::WavWriter<BufWriter<File>>>>,
        level: Arc<AtomicU32>,
    }
    let writer = hound::WavWriter::create(
        path,
        hound::WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .map_err(|issue| format!("无法写入录音文件：{issue}"))?;
    let context = Box::new(Context {
        writer: Mutex::new(Some(writer)),
        level: Arc::clone(level),
    });
    let context_ptr = Box::into_raw(context);

    unsafe extern "C-unwind" fn io_proc(
        _device: u32,
        _now: std::ptr::NonNull<AudioTimeStamp>,
        input: std::ptr::NonNull<AudioBufferList>,
        _input_time: std::ptr::NonNull<AudioTimeStamp>,
        _output: std::ptr::NonNull<AudioBufferList>,
        _output_time: std::ptr::NonNull<AudioTimeStamp>,
        client: *mut std::ffi::c_void,
    ) -> i32 {
        if client.is_null() {
            return 0;
        }
        let context = &*(client as *const Context);
        let list = input.as_ref();
        let buffers = std::slice::from_raw_parts(
            list.mBuffers.as_ptr() as *const AudioBuffer,
            list.mNumberBuffers as usize,
        );
        let Ok(mut guard) = context.writer.lock() else {
            return 0;
        };
        let Some(writer) = guard.as_mut() else {
            return 0;
        };
        let mut peak = 0.0f32;
        for buffer in buffers {
            if buffer.mData.is_null() {
                continue;
            }
            let frames = buffer.mDataByteSize as usize / std::mem::size_of::<f32>();
            let samples = std::slice::from_raw_parts(buffer.mData as *const f32, frames);
            for sample in samples {
                peak = peak.max(sample.abs());
                let _ = writer.write_sample((*sample * i16::MAX as f32) as i16);
            }
        }
        store_level(&context.level, peak);
        0
    }

    let mut proc_id: objc2_core_audio::AudioDeviceIOProcID = None;
    let status = AudioDeviceCreateIOProcID(
        aggregate_id,
        Some(io_proc),
        context_ptr as *mut std::ffi::c_void,
        std::ptr::NonNull::new(&mut proc_id).unwrap(),
    );
    if status != 0 {
        AudioHardwareDestroyAggregateDevice(aggregate_id);
        AudioHardwareDestroyProcessTap(tap_id);
        drop(Box::from_raw(context_ptr));
        return Err(format!("无法启动音频回调（错误 {status}）"));
    }
    let status = AudioDeviceStart(aggregate_id, proc_id);
    if status != 0 {
        AudioDeviceDestroyIOProcID(aggregate_id, proc_id);
        AudioHardwareDestroyAggregateDevice(aggregate_id);
        AudioHardwareDestroyProcessTap(tap_id);
        drop(Box::from_raw(context_ptr));
        return Err(format!("无法开始捕获音频（错误 {status}）"));
    }

    while !stop.load(Ordering::Relaxed) {
        thread::sleep(std::time::Duration::from_millis(80));
    }

    let _ = AudioDeviceStop(aggregate_id, proc_id);
    let _ = AudioDeviceDestroyIOProcID(aggregate_id, proc_id);
    let _ = AudioHardwareDestroyAggregateDevice(aggregate_id);
    let _ = AudioHardwareDestroyProcessTap(tap_id);

    let context = Box::from_raw(context_ptr);
    if let Ok(mut guard) = context.writer.lock()
        && let Some(wav) = guard.take()
    {
        let _ = wav.finalize();
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 命令
// ---------------------------------------------------------------------------

pub fn list_devices() -> Vec<String> {
    use cpal::traits::{DeviceTrait, HostTrait};
    let mut names: Vec<String> = cpal::default_host()
        .input_devices()
        .map(|devices| devices.filter_map(|device| device.name().ok()).collect())
        .unwrap_or_default();
    names.sort_by_key(|name| {
        let lower = name.to_lowercase();
        let builtin = lower.contains("macbook")
            || lower.contains("built-in")
            || lower.contains("内置")
            || lower.contains("mac studio")
            || lower.contains("imac");
        if builtin { 0 } else { 1 }
    });
    names.dedup();
    names
}

#[cfg(target_os = "macos")]
pub fn list_apps() -> Vec<AudioApp> {
    use objc2_app_kit::NSWorkspace;

    let workspace = unsafe { NSWorkspace::sharedWorkspace() };
    let apps = unsafe { workspace.runningApplications() };
    let mut out = Vec::new();
    for app in apps.iter() {
        let name = unsafe { app.localizedName() };
        let pid = unsafe { app.processIdentifier() };
        if pid <= 0 {
            continue;
        }
        let label = name
            .map(|name| name.to_string())
            .unwrap_or_else(|| format!("PID {pid}"));
        out.push(AudioApp { pid, name: label });
    }
    out.sort_by_key(|app| app.name.to_lowercase());
    out
}

#[cfg(not(target_os = "macos"))]
pub fn list_apps() -> Vec<AudioApp> {
    Vec::new()
}

pub fn start(
    state: &RecordingState,
    app: &AppHandle,
    specs: Vec<TrackSpec>,
) -> Result<(), String> {
    if specs.is_empty() {
        return Err("请至少添加一条音轨。".to_owned());
    }
    let mut guard = state
        .session
        .lock()
        .map_err(|_| "录音状态不可用。".to_owned())?;
    if guard.is_some() {
        return Err("已经在录音了。".to_owned());
    }
    let data_dir = crate::app_data_dir(app)?;
    let dir = data_dir
        .join("recordings")
        .join(crate::unix_now_ms().to_string());
    fs::create_dir_all(&dir).map_err(|issue| issue.to_string())?;

    let stop = Arc::new(AtomicBool::new(false));
    let mut tracks = Vec::new();
    for (index, spec) in specs.iter().enumerate() {
        let safe_index = index + 1;
        let label = if spec.label.trim().is_empty() {
            format!("音轨 {safe_index}")
        } else {
            spec.label.clone()
        };
        // 用来源名做文件名，历史列表里一眼能看出是哪条音轨。
        let file_stem: String = label
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
            .take(40)
            .collect();
        let path = dir.join(format!("{file_stem}-{safe_index}.wav"));
        if spec.kind == "app" {
            let Ok(pid) = spec.target.parse::<i32>() else {
                continue;
            };
            tracks.push(spawn_app(pid, &label, path, Arc::clone(&stop)));
        } else {
            tracks.push(spawn_input(
                Some(spec.target.clone()),
                &label,
                path,
                Arc::clone(&stop),
            ));
        }
    }

    if tracks.is_empty() {
        return Err("没有可用的录音来源。".to_owned());
    }
    *guard = Some(Session {
        stop,
        started: Instant::now(),
        tracks,
    });
    Ok(())
}

pub fn stop(state: &RecordingState) -> Result<RecordedTracks, String> {
    let mut guard = state
        .session
        .lock()
        .map_err(|_| "录音状态不可用。".to_owned())?;
    let Some(mut session) = guard.take() else {
        return Err("当前没有在录音。".to_owned());
    };
    session.stop.store(true, Ordering::Relaxed);

    let mut tracks = Vec::new();
    for mut track in session.tracks.drain(..) {
        if let Some(join) = track.join.take() {
            let _ = join.join();
        }
        if track.path.exists() {
            tracks.push(RecordedTrack {
                path: track.path.to_string_lossy().into_owned(),
                label: track.label.clone(),
            });
        }
    }
    Ok(RecordedTracks { tracks })
}

pub fn status(state: &RecordingState) -> RecordingStatus {
    let guard = match state.session.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return RecordingStatus {
                running: false,
                elapsed_ms: 0,
                tracks: Vec::new(),
            };
        }
    };
    match guard.as_ref() {
        Some(session) => RecordingStatus {
            running: true,
            elapsed_ms: session.started.elapsed().as_millis() as u64,
            tracks: session
                .tracks
                .iter()
                .map(|track| TrackLevel {
                    label: track.label.clone(),
                    level: load_level(&track.level),
                    error: track.error.lock().ok().and_then(|slot| slot.clone()),
                })
                .collect(),
        },
        None => RecordingStatus {
            running: false,
            elapsed_ms: 0,
            tracks: Vec::new(),
        },
    }
}
