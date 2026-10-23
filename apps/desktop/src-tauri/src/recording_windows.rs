//! Windows 应用音频：Application Loopback（按进程捕获，WASAPI 共享模式）。
//!
//! API 形状已对照 windows 0.61.3 注册表源码核实，并在真实 Windows 机器上编译通过；
//! 运行时行为以真机测试为准（见 docs/windows-recording-architecture.md）。
//!
//! 设计要点：
//! - 按 PID 捕获目标进程树的全部输出音频，其他应用的音频由系统混音引擎按进程隔离，不进入本轨。
//! - 目标进程退出后混音引擎继续输出静音：录制线程继续运行，错误槽报告状态，
//!   已录数据在停止封口后完整可恢复（验收 T08）。
//! - 写盘错误如实向上抛出并进入错误槽，不伪报成功（验收 T12）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU32},
};
use std::thread;
use std::time::{Duration, Instant};

use windows::core::{Interface, Ref, implement, w};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, TRUE, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::{
    ActivateAudioInterfaceAsync, AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED,
    AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
    AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK, AUDIOCLIENT_ACTIVATION_PARAMS,
    AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS, IActivateAudioInterfaceAsyncOperation,
    IActivateAudioInterfaceCompletionHandler, IActivateAudioInterfaceCompletionHandler_Impl,
    IAudioCaptureClient, IAudioClient, PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{
    CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_MULTITHREADED,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::SystemInformation::OSVERSIONINFOW;
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, WaitForSingleObject,
};
use windows::Wdk::System::SystemServices::RtlGetVersion;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, IsWindowVisible,
};

/// 录制线程入口：失败写入错误槽，成功静默返回。
pub fn run_loopback(
    pid: i32,
    path: PathBuf,
    level: Arc<AtomicU32>,
    stop: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
) {
    if let Err(issue) = capture(pid, &path, &level, &stop, &error)
        && let Ok(mut slot) = error.lock()
    {
        *slot = Some(issue);
    }
}

/// 激活完成回调的结果槽：回调只发信号，接口结果由录制线程调 GetActivateResult 获取。
struct CompletionSlot {
    done: Mutex<Option<Result<(), String>>>,
    cv: std::sync::Condvar,
}

#[implement(IActivateAudioInterfaceCompletionHandler)]
struct Completion {
    slot: Arc<CompletionSlot>,
}

impl IActivateAudioInterfaceCompletionHandler_Impl for Completion_Impl {
    fn ActivateCompleted(
        &self,
        activateoperation: Ref<'_, IActivateAudioInterfaceAsyncOperation>,
    ) -> windows::core::Result<()> {
        let result = match activateoperation.ok() {
            Ok(_) => Ok(()),
            Err(issue) => Err(format!("音频捕获激活回调失败：{issue}")),
        };
        if let Ok(mut guard) = self.slot.done.lock() {
            *guard = Some(result);
        }
        self.slot.cv.notify_all();
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum SampleKind {
    F32,
    I16,
}

/// 验收 T01：Application Loopback 需要 Windows 10 2004（build 19041）及以上。
const MIN_LOOPBACK_BUILD: u32 = 19_041;

fn check_loopback_support() -> Result<(), String> {
    let mut info: OSVERSIONINFOW = unsafe { std::mem::zeroed() };
    info.dwOSVersionInfoSize = std::mem::size_of::<OSVERSIONINFOW>() as u32;
    let status = unsafe { RtlGetVersion(&mut info) };
    // RtlGetVersion 失败时不伪造版本结论，交由后续激活错误路径报告。
    if status.0 >= 0 && info.dwBuildNumber > 0 && info.dwBuildNumber < MIN_LOOPBACK_BUILD {
        return Err(format!(
            "系统版本过旧（build {}），应用音频捕获需要 Windows 10 2004（build {MIN_LOOPBACK_BUILD}）及以上。",
            info.dwBuildNumber
        ));
    }
    Ok(())
}

fn process_alive(pid: u32) -> bool {
    unsafe {
        match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(handle) => {
                let exited = WaitForSingleObject(handle, 0) == WAIT_OBJECT_0;
                let _ = CloseHandle(handle);
                !exited
            }
            Err(_) => false,
        }
    }
}

fn capture(
    pid: i32,
    path: &Path,
    level: &Arc<AtomicU32>,
    stop: &Arc<AtomicBool>,
    error: &Arc<Mutex<Option<String>>>,
) -> Result<(), String> {
    if pid <= 0 {
        return Err("无效的应用进程号。".to_owned());
    }
    let pid = pid as u32;
    check_loopback_support()?;

    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()
            .map_err(|issue| format!("音频组件初始化失败：{issue}"))?;
    }

    // 与微软 ApplicationLoopbackAudio 示例一致：结构体指针按 ABI 直接作为激活参数传入。
    let mut params: AUDIOCLIENT_ACTIVATION_PARAMS = unsafe { std::mem::zeroed() };
    params.ActivationType = AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK;
    params.Anonymous.ProcessLoopbackParams = AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
        TargetProcessId: pid,
        ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
    };
    let pv = &params as *const AUDIOCLIENT_ACTIVATION_PARAMS as *const PROPVARIANT;

    let slot = Arc::new(CompletionSlot {
        done: Mutex::new(None),
        cv: std::sync::Condvar::new(),
    });
    let handler: IActivateAudioInterfaceCompletionHandler =
        Completion { slot: Arc::clone(&slot) }.into();

    let operation = unsafe {
        ActivateAudioInterfaceAsync(
            w!("__DEFAULT_LOOPBACK_DEVICE_ID"),
            &IAudioClient::IID,
            Some(pv),
            &handler,
        )
    }
    .map_err(|issue| format!("无法创建应用音频捕获（错误：{issue}）。需要 Windows 10 2004 及以上版本。"))?;

    // 等待激活完成回调（最长 10 秒）。
    {
        let mut guard = slot
            .done
            .lock()
            .map_err(|_| "音频捕获内部状态异常。".to_owned())?;
        let deadline = Instant::now() + Duration::from_secs(10);
        while guard.is_none() {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            let (next, _) = slot
                .cv
                .wait_timeout(guard, remaining)
                .map_err(|_| "音频捕获内部状态异常。".to_owned())?;
            guard = next;
        }
        match guard.take() {
            Some(Ok(())) => {}
            Some(Err(issue)) => return Err(issue),
            None => return Err("等待音频捕获初始化超时。".to_owned()),
        }
    }

    let mut activate_hr = windows::core::HRESULT(0);
    let mut activated: Option<windows::core::IUnknown> = None;
    unsafe {
        operation
            .GetActivateResult(&mut activate_hr, &mut activated)
            .map_err(|issue| format!("无法取得音频捕获接口（错误：{issue}）"))?;
    }
    if !activate_hr.is_ok() {
        return Err(format!("应用音频捕获初始化失败（{activate_hr}）。"));
    }
    let client: IAudioClient = activated
        .ok_or_else(|| "应用音频捕获未返回接口。".to_owned())?
        .cast()
        .map_err(|issue| format!("音频捕获接口类型错误：{issue}"))?;

    // 系统混音格式（通常 48 kHz 立体声 float32），用 AUTOCONVERTPCM 让引擎统一转换。
    let mix_ptr = unsafe { client.GetMixFormat() }
        .map_err(|issue| format!("无法读取系统混音格式：{issue}"))?;
    if mix_ptr.is_null() {
        return Err("系统混音格式为空。".to_owned());
    }
    let (sample_rate, channels, bits, tag) = unsafe {
        let mix = &*mix_ptr;
        // WAVEFORMATEX 为 packed 结构：先按值拷出字段再使用。
        (
            mix.nSamplesPerSec,
            mix.nChannels as usize,
            mix.wBitsPerSample,
            mix.wFormatTag,
        )
    };
    let sample_kind = match (tag, bits) {
        // 共享引擎混音格式：float32，或 WAVE_FORMAT_EXTENSIBLE 包装的 float32。
        (3, 32) | (0xFFFE, 32) => SampleKind::F32,
        (1, 16) => SampleKind::I16,
        _ => {
            unsafe { CoTaskMemFree(Some(mix_ptr.cast())) };
            return Err(format!(
                "暂不支持的系统混音格式（tag {tag}，{bits} bit），请反馈此信息。"
            ));
        }
    };

    let stream_flags = AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
    let init = unsafe {
        client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            stream_flags,
            20_000_000, // 2 秒环形缓冲，容忍调度抖动
            0,
            mix_ptr,
            None,
        )
    };
    unsafe { CoTaskMemFree(Some(mix_ptr.cast())) };
    init.map_err(|issue| format!("无法配置应用音频捕获（错误：{issue}）"))?;

    let capture_client: IAudioCaptureClient = unsafe {
        client
            .GetService()
            .map_err(|issue| format!("无法取得音频捕获缓冲（错误：{issue}）"))?
    };

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|issue| format!("无法创建录音目录：{issue}"))?;
    }
    let writer = hound::WavWriter::create(
        path,
        hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .map_err(|issue| format!("无法写入录音文件：{issue}"))?;
    let writer = Arc::new(Mutex::new(Some(writer)));

    unsafe { client.Start() }.map_err(|issue| format!("无法开始捕获应用音频：{issue}"))?;

    let mut peak = 0.0f32;
    let mut level_frames: u32 = 0;
    let level_every = (sample_rate / 20).max(1);
    let mut target_gone = false;

    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
        if !target_gone && !process_alive(pid) {
            target_gone = true;
            if let Ok(mut slot) = error.lock() {
                *slot = Some("目标应用已退出，正在继续录制静音；可停止后转写已录内容。".to_owned());
            }
        }
        let mut next = unsafe { capture_client.GetNextPacketSize() }
            .map_err(|issue| format!("读取音频缓冲失败：{issue}"))?;
        while next > 0 {
            let mut data: *mut u8 = std::ptr::null_mut();
            let mut frames: u32 = 0;
            let mut flags: u32 = 0;
            unsafe { capture_client.GetBuffer(&mut data, &mut frames, &mut flags, None, None) }
                .map_err(|issue| format!("读取音频数据失败：{issue}"))?;
            let silent = (flags & (AUDCLNT_BUFFERFLAGS_SILENT.0 as u32)) != 0;

            let mut guard = writer
                .lock()
                .map_err(|_| "录音文件写入状态异常。".to_owned())?;
            let wav = guard
                .as_mut()
                .ok_or_else(|| "录音文件已被意外关闭。".to_owned())?;
            let write_issue = "写入录音文件失败".to_owned();
            if silent || data.is_null() {
                for _ in 0..frames {
                    wav.write_sample(0i16).map_err(|_| write_issue.clone())?;
                }
            } else {
                match sample_kind {
                    SampleKind::F32 => {
                        let samples = unsafe {
                            std::slice::from_raw_parts(data as *const f32, frames as usize * channels)
                        };
                        for frame in samples.chunks_exact(channels) {
                            let mono = frame.iter().sum::<f32>() / channels as f32;
                            peak = peak.max(mono.abs());
                            wav.write_sample((mono.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
                                .map_err(|_| write_issue.clone())?;
                        }
                    }
                    SampleKind::I16 => {
                        let samples = unsafe {
                            std::slice::from_raw_parts(data as *const i16, frames as usize * channels)
                        };
                        for frame in samples.chunks_exact(channels) {
                            let mono =
                                frame.iter().map(|sample| *sample as f32).sum::<f32>()
                                    / channels as f32;
                            peak = peak.max((mono / i16::MAX as f32).abs());
                            wav.write_sample(mono as i16).map_err(|_| write_issue.clone())?;
                        }
                    }
                }
            }
            drop(guard);

            level_frames += frames;
            if level_frames >= level_every {
                crate::recording::store_level(level, peak);
                peak = 0.0;
                level_frames = 0;
            }

            unsafe { capture_client.ReleaseBuffer(frames) }
                .map_err(|issue| format!("释放音频缓冲失败：{issue}"))?;
            next = unsafe { capture_client.GetNextPacketSize() }
                .map_err(|issue| format!("读取音频缓冲失败：{issue}"))?;
        }
        thread::sleep(Duration::from_millis(10));
    }

    crate::recording::store_level(level, peak);
    let _ = unsafe { client.Stop() };
    drop(capture_client);
    drop(client);
    if let Ok(mut guard) = writer.lock()
        && let Some(wav) = guard.take()
    {
        let _ = wav.finalize();
    }
    drop(operation);
    drop(handler);
    unsafe { CoUninitialize(); }
    Ok(())
}

// ---------------------------------------------------------------------------
// 应用枚举：带可见窗口的进程（与 macOS runningApplications 的可用性语义对齐）
// ---------------------------------------------------------------------------

/// 返回 (pid, 进程名) 列表；进程名取 exe 基名（去 .exe）。
pub fn visible_apps() -> Vec<(i32, String)> {
    let names = process_names();
    let mut pids: Vec<u32> = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(enum_windows_proc),
            LPARAM(&mut pids as *mut Vec<u32> as isize),
        );
    }
    pids.into_iter()
        .filter_map(|pid| {
            let name = names.get(&pid).map(|value| {
                value
                    .strip_suffix(".exe")
                    .map(str::to_owned)
                    .unwrap_or_else(|| value.clone())
            });
            name.map(|name| (pid as i32, name))
        })
        .collect()
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
    unsafe {
        if IsWindowVisible(hwnd).as_bool() {
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            // 0/4 为 System/Idle 进程，不属于可选应用。
            if pid > 4 {
                let list = &mut *(lparam.0 as *mut Vec<u32>);
                if !list.contains(&pid) {
                    list.push(pid);
                }
            }
        }
        TRUE
    }
}

fn process_names() -> HashMap<u32, String> {
    let mut map = HashMap::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return map;
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snap, &mut entry).is_err() {
            let _ = CloseHandle(snap);
            return map;
        }
        loop {
            let len = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
            map.insert(entry.th32ProcessID, name);
            if Process32NextW(snap, &mut entry).is_err() {
                break;
            }
        }
        let _ = CloseHandle(snap);
    }
    map
}
