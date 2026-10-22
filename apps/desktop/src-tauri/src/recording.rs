//! 本地录音：麦克风（cpal）+ 指定应用的音频（Core Audio Process Tap）。
#![allow(unsafe_op_in_unsafe_fn)]
//!
//! 两条音轨各自写一个 WAV 文件，录完分别进入现有转写管线；
//! 界面按「麦克风 / 会议音频」两个独立结果呈现。

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
    mic: Option<MicTrack>,
    app: Option<AppTrack>,
}

struct MicTrack {
    path: PathBuf,
    level: Arc<AtomicU32>,
    join: Option<JoinHandle<()>>,
}

struct AppTrack {
    path: PathBuf,
    level: Arc<AtomicU32>,
    join: Option<JoinHandle<()>>,
    error: Arc<Mutex<Option<String>>>,
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
pub struct RecordingStatus {
    pub running: bool,
    pub elapsed_ms: u64,
    pub mic_level: f32,
    pub app_level: f32,
    pub app_error: Option<String>,
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

// ---------------------------------------------------------------------------
// 麦克风
// ---------------------------------------------------------------------------

fn spawn_microphone(device_name: Option<String>, path: PathBuf) -> Result<MicTrack, String> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();
    let device = match device_name.as_deref() {
        Some(name) if !name.is_empty() => host
            .input_devices()
            .map_err(|error| error.to_string())?
            .find(|device| device.name().map(|value| value == name).unwrap_or(false))
            .ok_or_else(|| format!("找不到麦克风「{name}」"))?,
        _ => host
            .default_input_device()
            .ok_or_else(|| "没有可用的麦克风。".to_owned())?,
    };
    let config = device.default_input_config().map_err(|error| {
        format!("无法读取麦克风配置（检查系统设置里的麦克风权限）：{error}")
    })?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let sample_format = config.sample_format();

    let level = Arc::new(AtomicU32::new(0.0f32.to_bits()));
    let stop = Arc::new(AtomicBool::new(false));

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let writer = hound::WavWriter::create(
        &path,
        hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .map_err(|error| format!("无法写入录音文件：{error}"))?;

    let mic_level = Arc::clone(&level);
    let mic_stop = Arc::clone(&stop);
    let join = thread::Builder::new()
        .name("recording-mic".to_owned())
        .spawn(move || {
            let writer = Arc::new(Mutex::new(Some(writer)));
            let mut config = config.config();
            config.buffer_size = cpal::BufferSize::Default;

            let sink = Arc::clone(&writer);
            let lvl = Arc::clone(&mic_level);
            let mut peak: f32 = 0.0;
            let mut peak_frames: usize = 0;

            let stream = match sample_format {
                cpal::SampleFormat::F32 => device.build_input_stream(
                    &config,
                    move |data: &[f32], _| {
                        let mut guard = match sink.lock() {
                            Ok(guard) => guard,
                            Err(_) => return,
                        };
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
                    |error| eprintln!("microphone stream error: {error}"),
                    None,
                ),
                other => {
                    eprintln!("unsupported microphone sample format: {other:?}");
                    return;
                }
            };

            match stream {
                Ok(stream) => {
                    let _ = stream.play();
                    while !mic_stop.load(Ordering::Relaxed) {
                        thread::sleep(std::time::Duration::from_millis(50));
                    }
                    drop(stream);
                    if let Ok(mut guard) = writer.lock()
                        && let Some(wav) = guard.take()
                    {
                        let _ = wav.finalize();
                    }
                }
                Err(error) => eprintln!("cannot start microphone stream: {error}"),
            }
        })
        .map_err(|error| error.to_string())?;

    Ok(MicTrack {
        path,
        level,
        join: Some(join),
    })
}

// ---------------------------------------------------------------------------
// 应用音频（Core Audio Process Tap）
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn spawn_app_audio(
    pid: Option<i32>,
    path: PathBuf,
) -> (Option<AppTrack>, Arc<AtomicBool>, Arc<AtomicU32>, Arc<Mutex<Option<String>>>) {
    let level = Arc::new(AtomicU32::new(0.0f32.to_bits()));
    let stop = Arc::new(AtomicBool::new(false));
    let error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let track = {
        let thread_path = path.clone();
        let thread_level = Arc::clone(&level);
        let thread_stop = Arc::clone(&stop);
        let thread_error = Arc::clone(&error);
        match thread::Builder::new()
            .name("recording-app".to_owned())
            .spawn(move || run_app_tap(pid, thread_path, thread_level, thread_stop, thread_error))
        {
            Ok(join) => Some(AppTrack {
                path,
                level: Arc::clone(&level),
                join: Some(join),
                error: Arc::clone(&error),
            }),
            Err(issue) => {
                if let Ok(mut slot) = error.lock() {
                    *slot = Some(issue.to_string());
                }
                None
            }
        }
    };
    (track, stop, level, error)
}

#[cfg(target_os = "macos")]
fn run_app_tap(
    pid: Option<i32>,
    path: PathBuf,
    level: Arc<AtomicU32>,
    stop: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
) {
    let result = unsafe { start_tap_session(pid, &path, &level, &stop) };
    if let Err(message) = result
        && let Ok(mut slot) = error.lock()
    {
        *slot = Some(message);
    }
}

#[cfg(target_os = "macos")]
unsafe fn start_tap_session(
    pid: Option<i32>,
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

    // 1) PID → AudioObjectID（进程音频对象）
    let mut obj_id: u32 = 0;
    if let Some(pid) = pid {
        let mut address = AudioObjectPropertyAddress {
            mSelector: u32::from_be_bytes(*b"id2p"),
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain,
        };
        let mut size = std::mem::size_of::<u32>() as u32;
        let status = AudioObjectGetPropertyData(
            1, // kAudioObjectSystemObject
            std::ptr::NonNull::new(&mut address).unwrap(),
            std::mem::size_of::<i32>() as u32,
            &pid as *const i32 as *const std::ffi::c_void,
            std::ptr::NonNull::new(&mut size).unwrap(),
            std::ptr::NonNull::new(&mut obj_id as *mut u32 as *mut std::ffi::c_void).unwrap(),
        );
        if status != 0 {
            return Err(format!("无法定位该应用的音频（错误 {status}）"));
        }
    }

    // 2) CATapDescription（指定进程；失败时退化为全系统输出）
    let (description, tap_scope) = if obj_id != 0 {
        let numbers = NSArray::from_retained_slice(&[NSNumber::new_u32(obj_id)]);
        let desc = CATapDescription::initStereoMixdownOfProcesses(
            CATapDescription::alloc(),
            &numbers,
        );
        (desc, "app")
    } else {
        let empty = NSArray::<NSNumber>::new();
        let desc = CATapDescription::initStereoGlobalTapButExcludeProcesses(
            CATapDescription::alloc(),
            &empty,
        );
        (desc, "system")
    };
    description.setName(&NSString::from_str("Local Transcribe recording"));
    description.setPrivate(true);

    // 3) 创建 tap
    let mut tap_id: u32 = 0;
    let status = AudioHardwareCreateProcessTap(Some(&description), &mut tap_id as *mut u32);
    if status != 0 {
        return Err(format!(
            "无法创建系统音频捕获（错误 {status}，可能需要「系统音频录制」权限）"
        ));
    }
    let _ = tap_scope;

    // 4) 聚合成一个可用设备
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

    // 5) WAV 写入器 + IOProc
    struct Context {
        writer: Mutex<Option<hound::WavWriter<BufWriter<File>>>>,
        level: Arc<AtomicU32>,
        peak: f32,
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
        peak: 0.0,
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
        for buffer in buffers {
            if buffer.mData.is_null() {
                continue;
            }
            let frames = buffer.mDataByteSize as usize / std::mem::size_of::<f32>();
            let samples = std::slice::from_raw_parts(buffer.mData as *const f32, frames);
            for sample in samples {
                let _ = writer.write_sample((*sample * i16::MAX as f32) as i16);
            }
        }
        store_level(&context.level, 0.5);
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

#[cfg(not(target_os = "macos"))]
fn spawn_app_audio(
    _pid: Option<i32>,
    _path: PathBuf,
) -> (Option<AppTrack>, Arc<AtomicBool>, Arc<AtomicU32>, Arc<Mutex<Option<String>>>) {
    let level = Arc::new(AtomicU32::new(0.0f32.to_bits()));
    let stop = Arc::new(AtomicBool::new(false));
    let error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(Some(
        "录制应用音频目前仅支持 macOS。".to_owned(),
    )));
    (None, stop, level, error)
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
    // 内置麦克风优先，避免默认选中投送设备或手机麦克风。
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
    use objc2_foundation::NSString;

    let workspace = unsafe { NSWorkspace::sharedWorkspace() };
    let apps = unsafe { workspace.runningApplications() };
    let mut out = Vec::new();
    for app in apps.iter() {
        let name = unsafe { app.localizedName() };
        let pid = unsafe { app.processIdentifier() };
        if pid <= 0 {
            continue;
        }
        let label: String = name
            .map(|name| name.to_string())
            .unwrap_or_else(|| format!("PID {pid}"));
        let _ = NSString::from_str(&label);
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
    mic: Option<String>,
    app_pid: Option<i32>,
) -> Result<(), String> {
    let mut guard = state
        .session
        .lock()
        .map_err(|_| "录音状态不可用。".to_owned())?;
    if guard.is_some() {
        return Err("已经在录音了。".to_owned());
    }
    let data_dir = crate::app_data_dir(app)?;
    let stamp = crate::unix_now_ms();
    let dir = data_dir.join("recordings").join(stamp.to_string());
    fs::create_dir_all(&dir).map_err(|issue| issue.to_string())?;

    let stop = Arc::new(AtomicBool::new(false));
    let mut session = Session {
        stop: Arc::clone(&stop),
        started: Instant::now(),
        mic: None,
        app: None,
    };

    if let Some(device) = mic.clone() {
        session.mic = Some(spawn_microphone(Some(device), dir.join("microphone.wav"))?);
    }

    let wants_app = app_pid.is_some();
    if wants_app {
        let (track, app_stop, _level, error) = spawn_app_audio(app_pid, dir.join("app-audio.wav"));
        session.app = track;
        if session.app.is_none() {
            let message = error
                .lock()
                .ok()
                .and_then(|slot| slot.clone())
                .unwrap_or_else(|| "无法捕获应用音频。".to_owned());
            if session.mic.is_none() {
                return Err(message);
            }
            eprintln!("app audio unavailable: {message}");
        }
        // 应用音轨用同一个停止信号
        let _ = app_stop;
    }

    if session.mic.is_none() && session.app.is_none() {
        return Err("请至少选择一个录音来源。".to_owned());
    }

    *guard = Some(session);
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
    if let Some(mut mic) = session.mic.take() {
        if let Some(join) = mic.join.take() {
            let _ = join.join();
        }
        if mic.path.exists() {
            tracks.push(RecordedTrack {
                path: mic.path.to_string_lossy().into_owned(),
                label: "麦克风".to_owned(),
            });
        }
    }
    if let Some(mut app) = session.app.take() {
        if let Some(join) = app.join.take() {
            let _ = join.join();
        }
        if app.path.exists() {
            let label = if let Ok(slot) = app.error.lock() {
                match slot.as_deref() {
                    Some(message) => format!("会议音频（{message}）"),
                    None => "会议音频".to_owned(),
                }
            } else {
                "会议音频".to_owned()
            };
            tracks.push(RecordedTrack {
                path: app.path.to_string_lossy().into_owned(),
                label,
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
                mic_level: 0.0,
                app_level: 0.0,
                app_error: None,
            };
        }
    };
    match guard.as_ref() {
        Some(session) => RecordingStatus {
            running: true,
            elapsed_ms: session.started.elapsed().as_millis() as u64,
            mic_level: session.mic.as_ref().map(|mic| load_level(&mic.level)).unwrap_or(0.0),
            app_level: session.app.as_ref().map(|app| load_level(&app.level)).unwrap_or(0.0),
            app_error: session
                .app
                .as_ref()
                .and_then(|app| app.error.lock().ok().and_then(|slot| slot.clone())),
        },
        None => RecordingStatus {
            running: false,
            elapsed_ms: 0,
            mic_level: 0.0,
            app_level: 0.0,
            app_error: None,
        },
    }
}
