use std::{
    ffi::{CStr, CString},
    os::raw::c_void,
    path::PathBuf,
    sync::atomic::{AtomicBool, AtomicI32, Ordering},
    thread,
    time::Duration,
};

use once_cell::sync::Lazy;
use widestring::U16CString;

use windows::{
    core::PCSTR,
    Win32::{
        Foundation::{BOOL, HINSTANCE, MAX_PATH},
        Security::SECURITY_ATTRIBUTES,
        System::{
            LibraryLoader::{GetModuleFileNameW, GetModuleHandleA, GetProcAddress},
            SystemServices::DLL_PROCESS_ATTACH,
            Threading::CreateThread,
        },
    },
};

static TARGET_FPS: AtomicI32 = AtomicI32::new(120);
static VSYNC_ENABLED: AtomicBool = AtomicBool::new(false);
static IL2CPP_RESOLVE_ICALL: Lazy<CString> = Lazy::new(|| CString::new("il2cpp_resolve_icall").unwrap());
static SET_TFR_ICALL: Lazy<CString> =
    Lazy::new(|| CString::new("UnityEngine.Application::set_targetFrameRate(System.Int32)").unwrap());
static SET_VSYNC_ICALL: Lazy<CString> =
    Lazy::new(|| CString::new("UnityEngine.QualitySettings::set_vSyncCount(System.Int32)").unwrap());

type Il2cppResolveIcallFn = unsafe extern "C" fn(name: *const i8) -> *const c_void;
type SetTargetFrameRateFn = extern "C" fn(i32);
type SetVSyncCountFn = extern "C" fn(i32);

static mut ORIGINAL_SET_TARGETFRAMERATE: Option<SetTargetFrameRateFn> = None;
static mut ORIGINAL_SET_VSYNCCOUNT: Option<SetVSyncCountFn> = None;

extern "C" fn set_targetFrameRate_hook(mut value: i32) {
    let fps = TARGET_FPS.load(Ordering::Relaxed);
    if fps > 0 {
        value = fps;
    }
    unsafe {
        if let Some(orig) = ORIGINAL_SET_TARGETFRAMERATE {
            (orig)(value);
        }
    }
}

extern "C" fn set_vSyncCount_hook(mut value: i32) {
    // Force vsync off unless explicitly enabled
    if !VSYNC_ENABLED.load(Ordering::Relaxed) {
        value = 0;
    }
    unsafe {
        if let Some(orig) = ORIGINAL_SET_VSYNCCOUNT {
            (orig)(value);
        }
    }
}

unsafe fn get_game_dir() -> Option<PathBuf> {
    let mut buf = [0u16; MAX_PATH as usize];
    let len = GetModuleFileNameW(HINSTANCE::default(), &mut buf) as usize;
    if len == 0 || len >= buf.len() {
        return None;
    }
    let s = U16CString::from_vec_unchecked(buf[..len].to_vec());
    let path = PathBuf::from(s.to_string().ok()?);
    Some(path.parent()?.to_path_buf())
}

fn read_target_fps_from_file(dir: &PathBuf) {
    let path = dir.join("unlocker_fps.txt");
    if let Ok(text) = std::fs::read_to_string(path) {
        if let Ok(v) = text.trim().parse::<i32>() {
            if v > 0 {
                TARGET_FPS.store(v, Ordering::Relaxed);
            }
        }
    }
}

fn read_vsync_from_file(dir: &PathBuf) {
    let path = dir.join("unlocker_vsync.txt");
    if let Ok(text) = std::fs::read_to_string(path) {
        let t = text.trim().to_ascii_lowercase();
        let enable = matches!(t.as_str(), "1" | "true" | "on" | "enable" | "enabled");
        VSYNC_ENABLED.store(enable, Ordering::Relaxed);
    } else {
        // default: disabled
        VSYNC_ENABLED.store(false, Ordering::Relaxed);
    }
}

use windows::Win32::Foundation::HMODULE;

unsafe fn wait_for_module(name: &CStr, max_wait_ms: u64) -> Option<HMODULE> {
    let start = std::time::Instant::now();
    loop {
        let h = GetModuleHandleA(PCSTR(name.as_ptr() as _)).ok();
        if let Some(hm) = h {
            if hm.0 != 0 { return Some(hm); }
        }
        if start.elapsed() > Duration::from_millis(max_wait_ms) {
            return None;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

unsafe fn hook_set_target_frame_rate() {
    // Read desired FPS
    if let Some(dir) = get_game_dir() {
        read_target_fps_from_file(&dir);
        read_vsync_from_file(&dir);
    }

    // Wait for GameAssembly.dll to be present
    let game_assembly = CString::new("GameAssembly.dll").unwrap();
    if wait_for_module(&game_assembly, 60_000).is_none() {
        return;
    }

    // Resolve il2cpp_resolve_icall
    let h_game = match GetModuleHandleA(PCSTR(game_assembly.as_ptr() as _)) {
        Ok(h) => h,
        Err(_) => HMODULE(0),
    };
    if h_game.0 == 0 {
        return;
    }

    let proc = GetProcAddress(h_game, PCSTR(IL2CPP_RESOLVE_ICALL.as_ptr() as _));
    if proc.is_none() {
        return;
    }
    let il2cpp_resolve_icall: Il2cppResolveIcallFn = std::mem::transmute(proc.unwrap());

    // Resolve UnityEngine.Application::set_targetFrameRate
    let tfr_addr = il2cpp_resolve_icall(SET_TFR_ICALL.as_ptr());
    if tfr_addr.is_null() {
        return;
    }

    // Resolve UnityEngine.QualitySettings::set_vSyncCount
    let vsync_addr = il2cpp_resolve_icall(SET_VSYNC_ICALL.as_ptr());

    // Install MinHook detour
    let orig = match minhook::MinHook::create_hook(tfr_addr as *mut c_void, set_targetFrameRate_hook as *mut c_void) {
        Ok(p) => p,
        Err(_) => return,
    };
    if minhook::MinHook::enable_hook(tfr_addr as *mut c_void).is_err() {
        return;
    }
    ORIGINAL_SET_TARGETFRAMERATE = Some(std::mem::transmute(orig));

    // Hook vsync setter if available
    if !vsync_addr.is_null() {
        if let Ok(p) = minhook::MinHook::create_hook(vsync_addr as *mut c_void, set_vSyncCount_hook as *mut c_void) {
            if minhook::MinHook::enable_hook(vsync_addr as *mut c_void).is_ok() {
                ORIGINAL_SET_VSYNCCOUNT = Some(std::mem::transmute(p));
            }
        }
    }

    // Apply immediately once
    if let Some(orig_fn) = ORIGINAL_SET_TARGETFRAMERATE {
        let fps = TARGET_FPS.load(Ordering::Relaxed);
        if fps > 0 {
            orig_fn(fps);
        }
    }

    // Also apply vsync off once
    if let Some(vsync_fn) = unsafe { ORIGINAL_SET_VSYNCCOUNT } {
        let enabled = VSYNC_ENABLED.load(Ordering::Relaxed);
        vsync_fn(if enabled { 1 } else { 0 });
    }
}

unsafe extern "system" fn init_thread(_: *mut c_void) -> u32 {
    hook_set_target_frame_rate();
    0
}

#[no_mangle]
pub unsafe extern "system" fn DllMain(hinst: HINSTANCE, reason: u32, _reserved: *mut c_void) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        let _ = CreateThread(
            Some(std::ptr::null::<SECURITY_ATTRIBUTES>()),
            0,
            Some(init_thread),
            None,
            windows::Win32::System::Threading::THREAD_CREATION_FLAGS(0),
            None,
        );
        let _ = hinst;
    }
    BOOL(1)
}
