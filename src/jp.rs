use std::{ffi::CStr, io::Write, path::PathBuf};

use widestring::U16CString;
use windows::{
    core::PCSTR,
    Win32::{
        Foundation::{CloseHandle, HMODULE, MAX_PATH},
        System::{
            Diagnostics::{
                Debug::WriteProcessMemory,
                ToolHelp::{
                    CreateToolhelp32Snapshot, Process32First, Process32Next, PROCESSENTRY32, TH32CS_SNAPPROCESS,
                },
            },
            LibraryLoader::{GetModuleHandleA, GetProcAddress},
            Memory::{VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, PAGE_READWRITE},
            ProcessStatus::K32GetModuleFileNameExW,
            Threading::{
                CreateRemoteThread, OpenProcess, WaitForSingleObject, PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION,
                PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE, INFINITE,
            },
        },
    },
};

fn find_pid_by_names(names: &[&str]) -> Option<u32> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
        let mut entry = PROCESSENTRY32::default();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32>() as u32;

        let mut res = Process32First(snapshot, &mut entry);
        while res.is_ok() {
            let name = CStr::from_ptr(entry.szExeFile.as_ptr() as *const i8);
            let s = name.to_string_lossy().to_lowercase();
            if names.iter().any(|n| s == n.to_lowercase()) {
                let _ = CloseHandle(snapshot);
                return Some(entry.th32ProcessID);
            }
            res = Process32Next(snapshot, &mut entry);
        }
        let _ = CloseHandle(snapshot);
    }
    None
}

fn get_process_image_path(pid: u32) -> Option<PathBuf> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid).ok()?;
        let mut buf = [0u16; MAX_PATH as usize];
        let len = K32GetModuleFileNameExW(handle, HMODULE(0), &mut buf) as usize;
        if len == 0 {
            let _ = CloseHandle(handle);
            return None;
        }
        let path = U16CString::from_vec_unchecked(buf[..len].to_vec()).to_string().ok()?;
        let _ = CloseHandle(handle);
        Some(PathBuf::from(path))
    }
}

fn write_target_fps(game_dir: &PathBuf, fps: i32) -> std::io::Result<()> {
    let path = game_dir.join("unlocker_fps.txt");
    let mut f = std::fs::File::create(path)?;
    write!(f, "{}", fps)?;
    Ok(())
}

fn write_vsync(game_dir: &PathBuf, enable: bool) -> std::io::Result<()> {
    let path = game_dir.join("unlocker_vsync.txt");
    let mut f = std::fs::File::create(path)?;
    write!(f, "{}", if enable {"1"} else {"0"})?;
    Ok(())
}

fn inject_dll(pid: u32, dll_path: &PathBuf) -> windows::core::Result<()> {
    unsafe {
        let proc = OpenProcess(
            PROCESS_CREATE_THREAD | PROCESS_QUERY_INFORMATION | PROCESS_VM_OPERATION | PROCESS_VM_WRITE | PROCESS_VM_READ,
            false,
            pid,
        )?;

        let dll_w = U16CString::from_str(dll_path.as_os_str().to_string_lossy().as_ref()).unwrap();
        let alloc = VirtualAllocEx(proc, None, ((dll_w.len() + 1) * 2) as usize, MEM_COMMIT, PAGE_READWRITE);
        if alloc.is_null() {
            let _ = CloseHandle(proc);
            return Err(windows::core::Error::from_win32());
        }

        let write_ok = WriteProcessMemory(proc, alloc, dll_w.as_ptr() as _, ((dll_w.len() + 1) * 2) as usize, None).is_ok();
        if !write_ok {
            VirtualFreeEx(proc, alloc, 0, MEM_RELEASE).ok();
            let _ = CloseHandle(proc);
            return Err(windows::core::Error::from_win32());
        }

        let k32_name = std::ffi::CString::new("kernel32.dll").unwrap();
        let k32 = GetModuleHandleA(PCSTR(k32_name.as_ptr() as _))?;
        let loadlib = std::ffi::CString::new("LoadLibraryW").unwrap();
        let load_library_w = GetProcAddress(k32, PCSTR(loadlib.as_ptr() as _))
            .ok_or_else(|| windows::core::Error::from_win32())?;

        let thread = CreateRemoteThread(proc, None, 0, Some(std::mem::transmute(load_library_w)), Some(alloc), 0, None)?;
        let _ = WaitForSingleObject(thread, INFINITE);

        VirtualFreeEx(proc, alloc, 0, MEM_RELEASE).ok();
        let _ = CloseHandle(thread);
        let _ = CloseHandle(proc);

        Ok(())
    }
}

fn main() {
    // Defaults; change via flags
    let mut fps = 120i32;
    let mut vsync: Option<bool> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--fps" => {
                if let Some(v) = args.next() {
                    if let Ok(n) = v.parse::<i32>() { fps = n.max(1); }
                }
            }
            "--vsync" => {
                if let Some(v) = args.next() {
                    let vl = v.to_ascii_lowercase();
                    let enable = matches!(vl.as_str(), "1"|"on"|"true"|"enable"|"enabled");
                    let disable = matches!(vl.as_str(), "0"|"off"|"false"|"disable"|"disabled");
                    if enable { vsync = Some(true); }
                    else if disable { vsync = Some(false); }
                }
            }
            _ => {}
        }
    }

    let names = ["umamusume.exe", "umamusumeprettyderby_jpn.exe"];
    let pid = match find_pid_by_names(&names) {
        Some(pid) => pid,
        None => {
            eprintln!("JP process not found (umamusume.exe / umamusumeprettyderby_jpn.exe). Start the game first.");
            std::process::exit(1);
        }
    };

    let exe_path = match get_process_image_path(pid) {
        Some(p) => p,
        None => {
            eprintln!("Could not get game image path.");
            std::process::exit(1);
        }
    };
    let game_dir = exe_path.parent().unwrap().to_path_buf();

    if let Err(e) = write_target_fps(&game_dir, fps) {
        eprintln!("Failed writing target FPS file: {}", e);
        // Continue anyway
    }

    if let Some(v) = vsync {
        if let Err(e) = write_vsync(&game_dir, v) {
            eprintln!("Failed writing vsync file: {}", e);
        }
    }

    let dll_path = std::env::current_exe().unwrap().parent().unwrap().join("uma_unlocker.dll");
    if !dll_path.exists() {
        eprintln!("uma_unlocker.dll not found next to the injector.");
        std::process::exit(1);
    }

    if let Err(e) = inject_dll(pid, &dll_path) {
        eprintln!("Injection failed: {:?}", e);
        std::process::exit(1);
    }

    if let Some(v) = vsync {
        println!("Injected. Target FPS = {}, VSync = {}", fps, if v {"on"} else {"off"});
    } else {
        println!("Injected. Target FPS = {}", fps);
    }
}
