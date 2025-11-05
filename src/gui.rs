#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{ffi::{CStr, CString}, io::Write, mem::size_of, path::PathBuf, ptr::{null, null_mut}};
use widestring::{U16CString, U16CStr};
use windows::core::{PCSTR, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HINSTANCE, HWND, LPARAM, LRESULT, MAX_PATH, WPARAM};
use windows::Win32::Graphics::Gdi::{GetStockObject, DEFAULT_GUI_FONT};
use windows::Win32::System::Diagnostics::Debug::WriteProcessMemory;
use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32First, Process32Next, PROCESSENTRY32, TH32CS_SNAPPROCESS};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetModuleHandleA, GetProcAddress};
use windows::Win32::System::Memory::{VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, PAGE_READWRITE};
use windows::Win32::System::ProcessStatus::K32GetModuleFileNameExW;
use windows::Win32::System::Threading::{CreateRemoteThread, OpenProcess, WaitForSingleObject, PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE, INFINITE};
use windows::Win32::UI::WindowsAndMessaging::*;

const ID_RADIO_JP: i32 = 1001;
const ID_RADIO_GL: i32 = 1002;
const ID_EDIT_FPS: i32 = 1003;
const ID_CHECK_VSYNC: i32 = 1004;
const ID_BTN_INJECT: i32 = 1005;
const ID_BTN_QUIT: i32 = 1006;

fn to_w(s: &str) -> U16CString { U16CString::from_str(s).unwrap() }

unsafe fn set_default_font(hwnd: HWND) {
    let font = GetStockObject(DEFAULT_GUI_FONT);
    SendMessageW(hwnd, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
}

fn find_pid_by_names(names: &[&str]) -> Option<u32> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
        let mut entry = PROCESSENTRY32::default();
        entry.dwSize = size_of::<PROCESSENTRY32>() as u32;
        let mut res = Process32First(snapshot, &mut entry);
        while res.is_ok() {
            let name = CStr::from_ptr(entry.szExeFile.as_ptr() as *const i8);
            let s = name.to_string_lossy().to_lowercase();
            if names.iter().any(|n| s == n.to_lowercase()) { let _ = CloseHandle(snapshot); return Some(entry.th32ProcessID); }
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
        let len = K32GetModuleFileNameExW(handle, HINSTANCE(0), &mut buf) as usize;
        if len == 0 { let _ = CloseHandle(handle); return None; }
        let path = String::from_utf16_lossy(&buf[..len]);
        let _ = CloseHandle(handle);
        Some(PathBuf::from(path))
    }
}

fn write_target_fps(game_dir: &PathBuf, fps: i32) -> std::io::Result<()> { let path = game_dir.join("unlocker_fps.txt"); let mut f = std::fs::File::create(path)?; write!(f, "{}", fps)?; Ok(()) }
fn write_vsync(game_dir: &PathBuf, enable: bool) -> std::io::Result<()> { let path = game_dir.join("unlocker_vsync.txt"); let mut f = std::fs::File::create(path)?; write!(f, "{}", if enable {"1"} else {"0"})?; Ok(()) }

fn inject_dll(pid: u32, dll_path: &PathBuf) -> windows::core::Result<()> {
    unsafe {
        let proc = OpenProcess(PROCESS_CREATE_THREAD | PROCESS_QUERY_INFORMATION | PROCESS_VM_OPERATION | PROCESS_VM_WRITE | PROCESS_VM_READ, false, pid)?;
        let dll_w = to_w(&dll_path.as_os_str().to_string_lossy());
        let alloc = VirtualAllocEx(proc, None, ((dll_w.len() + 1) * 2) as usize, MEM_COMMIT, PAGE_READWRITE);
        if alloc.is_null() { let _ = CloseHandle(proc); return Err(windows::core::Error::from_win32()); }
        if !WriteProcessMemory(proc, alloc, dll_w.as_ptr() as _, ((dll_w.len() + 1) * 2) as usize, None).is_ok() {
            VirtualFreeEx(proc, alloc, 0, MEM_RELEASE).ok(); let _ = CloseHandle(proc); return Err(windows::core::Error::from_win32());
        }
        let k32 = GetModuleHandleA(PCSTR(CString::new("kernel32.dll").unwrap().as_ptr() as _))?;
        let load_library_w = GetProcAddress(k32, PCSTR(CString::new("LoadLibraryW").unwrap().as_ptr() as _)).ok_or_else(|| windows::core::Error::from_win32())?;
        let thread = CreateRemoteThread(proc, None, 0, Some(std::mem::transmute(load_library_w)), Some(alloc), 0, None)?;
        let _ = WaitForSingleObject(thread, INFINITE);
        VirtualFreeEx(proc, alloc, 0, MEM_RELEASE).ok(); let _ = CloseHandle(thread); let _ = CloseHandle(proc); Ok(())
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            // Labels
            let lbl = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(U16CString::from_str("STATIC").unwrap().as_ptr()),
                PCWSTR(U16CString::from_str("Server:").unwrap().as_ptr()),
                WS_CHILD | WS_VISIBLE,
                10, 10, 60, 20,
                hwnd, HMENU(0), HINSTANCE(0), None);
            set_default_font(lbl);
            // Radio JP
            let rb_jp = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(U16CString::from_str("BUTTON").unwrap().as_ptr()),
                PCWSTR(U16CString::from_str("JP").unwrap().as_ptr()),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_AUTORADIOBUTTON as u32) | WS_GROUP,
                15, 35, 60, 22,
                hwnd, HMENU(ID_RADIO_JP as _), HINSTANCE(0), None);
            set_default_font(rb_jp);
            SendMessageW(rb_jp, BM_SETCHECK, WPARAM(1), LPARAM(0));
            // Radio GL
            let rb_gl = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(U16CString::from_str("BUTTON").unwrap().as_ptr()),
                PCWSTR(U16CString::from_str("Global").unwrap().as_ptr()),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_AUTORADIOBUTTON as u32),
                80, 35, 70, 22,
                hwnd, HMENU(ID_RADIO_GL as _), HINSTANCE(0), None);
            set_default_font(rb_gl);
            // FPS label + edit
            let lbl_fps = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(U16CString::from_str("STATIC").unwrap().as_ptr()),
                PCWSTR(U16CString::from_str("FPS:").unwrap().as_ptr()),
                WS_CHILD | WS_VISIBLE,
                190, 20, 40, 20,
                hwnd, HMENU(0), HINSTANCE(0), None);
            set_default_font(lbl_fps);
            let edit = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(U16CString::from_str("EDIT").unwrap().as_ptr()),
                PCWSTR(U16CString::from_str("120").unwrap().as_ptr()),
                WS_CHILD | WS_VISIBLE | WS_BORDER | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                230, 18, 100, 24,
                hwnd, HMENU(ID_EDIT_FPS as _), HINSTANCE(0), None);
            set_default_font(edit);
            let chk = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(U16CString::from_str("BUTTON").unwrap().as_ptr()),
                PCWSTR(U16CString::from_str("VSync").unwrap().as_ptr()),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                190, 55, 140, 22,
                hwnd, HMENU(ID_CHECK_VSYNC as _), HINSTANCE(0), None);
            set_default_font(chk);
            let btn_inj = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(U16CString::from_str("BUTTON").unwrap().as_ptr()),
                PCWSTR(U16CString::from_str("Inject").unwrap().as_ptr()),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                80, 110, 90, 30,
                hwnd, HMENU(ID_BTN_INJECT as _), HINSTANCE(0), None);
            set_default_font(btn_inj);
            let btn_q = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(U16CString::from_str("BUTTON").unwrap().as_ptr()),
                PCWSTR(U16CString::from_str("Quit").unwrap().as_ptr()),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                190, 110, 90, 30,
                hwnd, HMENU(ID_BTN_QUIT as _), HINSTANCE(0), None);
            set_default_font(btn_q);
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as i32;
            let code = ((wparam.0 >> 16) & 0xFFFF) as u16;
            if id == ID_BTN_QUIT && code == BN_CLICKED as u16 { PostQuitMessage(0); return LRESULT(0); }
            if id == ID_BTN_INJECT && code == BN_CLICKED as u16 {
                // Gather values
                let rb_jp = GetDlgItem(hwnd, ID_RADIO_JP);
                let is_jp = SendMessageW(rb_jp, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 as u32 == 1;
                let names: Vec<&str> = if is_jp { vec!["umamusume.exe", "umamusumeprettyderby_jpn.exe"] } else { vec!["umamusumeprettyderby.exe"] };
                // FPS
                let edit = GetDlgItem(hwnd, ID_EDIT_FPS);
                let len = GetWindowTextLengthW(edit);
                let mut buf: Vec<u16> = vec![0; (len as usize) + 1];
                let got = GetWindowTextW(edit, &mut buf);
                let fps_text = String::from_utf16_lossy(&buf[..got as usize]);
                let fps = fps_text.trim().parse::<i32>().unwrap_or(0).max(0);
                if fps <= 0 {
                    let t1 = U16CString::from_str("Please enter a valid FPS (> 0)").unwrap();
                    let t2 = U16CString::from_str("Uma FPS Unlocker").unwrap();
                    MessageBoxW(hwnd, PCWSTR(t1.as_ptr()), PCWSTR(t2.as_ptr()), MB_OK | MB_ICONERROR);
                    return LRESULT(0);
                }
                // VSync
                let chk = GetDlgItem(hwnd, ID_CHECK_VSYNC);
                let vsync_on = SendMessageW(chk, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 as u32 == 1;
                // Find process
                let e1 = U16CString::from_str("Game process not found. Start the game first.").unwrap();
                let e2 = U16CString::from_str("Uma FPS Unlocker").unwrap();
                let Some(pid) = find_pid_by_names(&names) else { MessageBoxW(hwnd, PCWSTR(e1.as_ptr()), PCWSTR(e2.as_ptr()), MB_OK | MB_ICONERROR); return LRESULT(0); };
                // Game dir
                let e3 = U16CString::from_str("Could not get game image path").unwrap();
                let e4 = U16CString::from_str("Uma FPS Unlocker").unwrap();
                let Some(exe_path) = get_process_image_path(pid) else { MessageBoxW(hwnd, PCWSTR(e3.as_ptr()), PCWSTR(e4.as_ptr()), MB_OK | MB_ICONERROR); return LRESULT(0); };
                let game_dir = exe_path.parent().unwrap().to_path_buf();
                let e5 = U16CString::from_str("Failed writing FPS file").unwrap();
                let e6 = U16CString::from_str("Failed writing VSync file").unwrap();
                let cap = U16CString::from_str("Uma FPS Unlocker").unwrap();
                if let Err(_) = write_target_fps(&game_dir, fps) { MessageBoxW(hwnd, PCWSTR(e5.as_ptr()), PCWSTR(cap.as_ptr()), MB_OK | MB_ICONERROR); return LRESULT(0); }
                if let Err(_) = write_vsync(&game_dir, vsync_on) { MessageBoxW(hwnd, PCWSTR(e6.as_ptr()), PCWSTR(cap.as_ptr()), MB_OK | MB_ICONERROR); return LRESULT(0); }
                let dll_path = std::env::current_exe().unwrap().parent().unwrap().join("uma_unlocker.dll");
                let e7 = U16CString::from_str("uma_unlocker.dll not found next to the GUI executable").unwrap();
                if !dll_path.exists() { MessageBoxW(hwnd, PCWSTR(e7.as_ptr()), PCWSTR(cap.as_ptr()), MB_OK | MB_ICONERROR); return LRESULT(0); }
                match inject_dll(pid, &dll_path) {
                    Ok(_) => { let msg = format!("Injected. Target FPS = {}, VSync = {}", fps, if vsync_on {"on"} else {"off"}); let w = to_w(&msg); MessageBoxW(hwnd, PCWSTR(w.as_ptr()), PCWSTR(cap.as_ptr()), MB_OK | MB_ICONINFORMATION); },
                    Err(_) => { let e8 = U16CString::from_str("Injection failed").unwrap(); MessageBoxW(hwnd, PCWSTR(e8.as_ptr()), PCWSTR(cap.as_ptr()), MB_OK | MB_ICONERROR); },
                }
                return LRESULT(0);
            }
            LRESULT(0)
        }
        WM_DESTROY => { PostQuitMessage(0); LRESULT(0) }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

fn main() {
    unsafe {
        let hmodule = GetModuleHandleW(None).unwrap();
        let hinstance: HINSTANCE = hmodule.into();
        let class_name_u = U16CString::from_str("UmaUnlockerClass").unwrap();
        let class_name = PCWSTR(class_name_u.as_ptr());
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
            lpszClassName: class_name,
            ..Default::default()
        };
        RegisterClassW(&wc);
        let title = U16CString::from_str("Uma FPS Unlocker").unwrap();
        let hwnd = CreateWindowExW(WINDOW_EX_STYLE(0), class_name, PCWSTR(title.as_ptr()), WS_OVERLAPPEDWINDOW | WS_VISIBLE, CW_USEDEFAULT, CW_USEDEFAULT, 360, 200, None, None, hinstance, None);
        ShowWindow(hwnd, SW_SHOWDEFAULT);
        // UpdateWindow(hwnd) is optional; ShowWindow is enough here
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() { TranslateMessage(&msg); DispatchMessageW(&msg); }
    }
}
