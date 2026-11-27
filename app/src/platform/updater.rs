//! Portable-app update bootstrap for the downloadable Windows executable.
//!
//! The old app has no installer, so an update locates its running/saved copy,
//! closes it gracefully, replaces only `shukanji.exe`, and relaunches it. User
//! configuration, credentials, logs, and caches are never touched.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, id};

use windows_sys::Win32::Foundation::{
    CloseHandle, HANDLE, HWND, INVALID_HANDLE_VALUE, LPARAM, WAIT_OBJECT_0,
};
use windows_sys::Win32::Storage::FileSystem::{
    MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
    TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    WaitForSingleObject,
};
use windows_sys::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST,
    OPENFILENAMEW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, IDYES, MB_ICONERROR, MB_ICONINFORMATION, MB_OK,
    MB_YESNO, MessageBoxW, PostMessageW, WM_CLOSE,
};

const INSTALL_PATH_FILE: &str = "install-path";
const EXECUTABLE_NAME: &str = "shukanji.exe";
const SYNCHRONIZE_ACCESS: u32 = 0x0010_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupAction {
    Run,
    Exit,
}

/// Run before creating the GUI. Only release builds call this from `main`.
pub fn startup() -> StartupAction {
    let current = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => {
            show_error(&format!("Could not locate the running update:\n{error}"));
            return StartupAction::Run;
        }
    };
    let args: Vec<OsString> = std::env::args_os().collect();
    if args.iter().any(|arg| arg == "--no-update") {
        remember_install(&current);
        return StartupAction::Run;
    }
    let explicit = args
        .iter()
        .position(|arg| arg == "--update")
        .and_then(|index| args.get(index + 1))
        .map(PathBuf::from);
    if args.iter().any(|arg| arg == "--update") && explicit.is_none() {
        show_error("--update requires the path to the old shukanji.exe");
        return StartupAction::Run;
    }

    let target = explicit.clone().or_else(|| discover_install(&current));
    let target = match target {
        Some(target) => Some(target),
        None => {
            if !ask_to_select_old_install() {
                remember_install(&current);
                return StartupAction::Run;
            }
            pick_old_install()
        }
    };
    let Some(target) = target else {
        remember_install(&current);
        return StartupAction::Run;
    };
    if same_path(&current, &target) {
        remember_install(&current);
        return StartupAction::Run;
    }
    if let Err(error) = install_update(&current, &target) {
        show_error(&format!(
            "ShuKanji could not replace:\n{}\n\n{error}\n\nClose the old app and try again. Your settings were not changed.",
            target.display()
        ));
        return StartupAction::Run;
    }
    StartupAction::Exit
}

fn install_update(current: &Path, target: &Path) -> io::Result<()> {
    validate_target(target)?;
    close_running_copies(target)?;
    replace_executable(current, target)?;
    remember_install(target);
    Command::new(target).arg("--no-update").spawn()?;
    Ok(())
}

fn validate_target(target: &Path) -> io::Result<()> {
    if is_shukanji_executable(target) && target.is_file() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Select an existing file named shukanji.exe",
        ))
    }
}

fn is_shukanji_executable(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case(EXECUTABLE_NAME))
}

fn same_path(a: &Path, b: &Path) -> bool {
    let a = a.canonicalize().unwrap_or_else(|_| a.to_path_buf());
    let b = b.canonicalize().unwrap_or_else(|_| b.to_path_buf());
    a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy())
}

fn discover_install(current: &Path) -> Option<PathBuf> {
    if let Some(running) = running_installs()
        .into_iter()
        .map(|install| install.path)
        .find(|path| !same_path(current, path))
    {
        return Some(running);
    }
    if let Some(saved) = saved_install()
        && !same_path(current, &saved)
        && is_shukanji_executable(&saved)
        && saved.is_file()
    {
        return Some(saved);
    }
    let sibling = current.parent()?.join(EXECUTABLE_NAME);
    if !same_path(current, &sibling) && sibling.is_file() {
        Some(sibling)
    } else {
        None
    }
}

fn saved_install() -> Option<PathBuf> {
    let path = crate::persistence::paths::config_dir()?.join(INSTALL_PATH_FILE);
    let value = std::fs::read_to_string(path).ok()?;
    Some(PathBuf::from(value.trim()))
}

fn remember_install(path: &Path) {
    if let Some(dir) = crate::persistence::paths::config_dir()
        && std::fs::create_dir_all(&dir).is_ok()
    {
        let _ = std::fs::write(dir.join(INSTALL_PATH_FILE), path.to_string_lossy().as_bytes());
    }
}

fn replace_executable(current: &Path, target: &Path) -> io::Result<()> {
    if same_path(current, target) {
        return Ok(());
    }
    validate_target(target)?;
    let temp = target.with_file_name(format!(".shukanji-update-{}.tmp", id()));
    std::fs::copy(current, &temp)?;
    let source_wide = wide_path(&temp);
    let target_wide = wide_path(target);
    // The old process has exited, so an atomic replace avoids leaving a
    // partially copied executable if the copy itself is interrupted.
    let replaced = unsafe {
        MoveFileExW(
            source_wide.as_ptr(),
            target_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if replaced == 0 {
        let error = io::Error::last_os_error();
        let _ = std::fs::remove_file(temp);
        Err(error)
    } else {
        Ok(())
    }
}

#[derive(Debug)]
struct RunningInstall {
    pid: u32,
    path: PathBuf,
}

fn running_installs() -> Vec<RunningInstall> {
    let mut installs = Vec::new();
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return installs;
    }
    let snapshot = OwnedHandle(snapshot);
    let current_pid = id();
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut available = unsafe { Process32FirstW(snapshot.0, &mut entry) } != 0;
    while available {
        let pid = entry.th32ProcessID;
        let executable_name = wide_array_string(&entry.szExeFile);
        if pid != current_pid && executable_name.eq_ignore_ascii_case(EXECUTABLE_NAME) {
            if let Some(path) = process_path(pid) {
                installs.push(RunningInstall { pid, path });
            }
        }
        available = unsafe { Process32NextW(snapshot.0, &mut entry) } != 0;
    }
    installs
}

fn process_path(pid: u32) -> Option<PathBuf> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return None;
    }
    let process = OwnedHandle(process);
    let mut buffer = vec![0u16; 32_768];
    let mut length = buffer.len() as u32;
    let ok = unsafe {
        QueryFullProcessImageNameW(process.0, 0, buffer.as_mut_ptr(), &mut length)
    };
    if ok == 0 {
        return None;
    }
    buffer.truncate(length as usize);
    Some(PathBuf::from(OsString::from_wide(&buffer)))
}

fn close_running_copies(target: &Path) -> io::Result<()> {
    for install in running_installs()
        .into_iter()
        .filter(|install| same_path(&install.path, target))
    {
        close_process(install.pid)?;
    }
    Ok(())
}

fn close_process(pid: u32) -> io::Result<()> {
    let process = unsafe { OpenProcess(SYNCHRONIZE_ACCESS, 0, pid) };
    if process.is_null() {
        return Ok(()); // It exited between discovery and closing.
    }
    let process = OwnedHandle(process);
    unsafe {
        EnumWindows(Some(close_process_window), pid as LPARAM);
    }
    let result = unsafe { WaitForSingleObject(process.0, 8_000) };
    if result == WAIT_OBJECT_0 {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "The old ShuKanji window did not close within 8 seconds",
        ))
    }
}

unsafe extern "system" fn close_process_window(hwnd: HWND, pid: LPARAM) -> i32 {
    let mut window_pid = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut window_pid);
        if window_pid == pid as u32 {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
    }
    1
}

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn wide_array_string(value: &[u16]) -> String {
    let length = value.iter().position(|ch| *ch == 0).unwrap_or(value.len());
    String::from_utf16_lossy(&value[..length])
}

fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

fn wide_text(text: &str) -> Vec<u16> {
    std::ffi::OsStr::new(text)
        .encode_wide()
        .chain(Some(0))
        .collect()
}

use std::os::windows::ffi::{OsStrExt, OsStringExt};

fn show_error(message: &str) {
    let message = wide_text(message);
    let title = wide_text("ShuKanji update");
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn ask_to_select_old_install() -> bool {
    let message = wide_text(
        "Update an existing ShuKanji copy?\n\nYes: select the old shukanji.exe to replace and relaunch.\nNo: run this downloaded copy without updating another file.",
    );
    let title = wide_text("ShuKanji update");
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_YESNO | MB_ICONINFORMATION,
        ) == IDYES
    }
}

fn pick_old_install() -> Option<PathBuf> {
    let mut buffer = vec![0u16; 32_768];
    let filter = wide_text(
        "ShuKanji executable (shukanji.exe)\0shukanji.exe\0All executables (*.exe)\0*.exe\0",
    );
    let title = wide_text("Select the old shukanji.exe to update");
    let mut dialog = OPENFILENAMEW {
        lStructSize: size_of::<OPENFILENAMEW>() as u32,
        lpstrFilter: filter.as_ptr(),
        nFilterIndex: 1,
        lpstrFile: buffer.as_mut_ptr(),
        nMaxFile: buffer.len() as u32,
        lpstrTitle: title.as_ptr(),
        Flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    if unsafe { GetOpenFileNameW(&mut dialog) } == 0 {
        return None;
    }
    buffer.truncate(buffer.iter().position(|ch| *ch == 0).unwrap_or(buffer.len()));
    Some(PathBuf::from(OsString::from_wide(&buffer)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_shukanji_executable_is_an_update_target() {
        assert!(is_shukanji_executable(Path::new(r"C:\Apps\ShuKanji.exe")));
        assert!(!is_shukanji_executable(Path::new(r"C:\Apps\other.exe")));
    }

    #[test]
    fn windows_paths_compare_without_case_sensitive_path_changes() {
        assert!(same_path(
            Path::new(r"C:\Apps\Shukanji\shukanji.exe"),
            Path::new(r"c:\apps\shukanji\SHUKANJI.EXE"),
        ));
    }

    #[test]
    fn replacing_an_executable_preserves_the_download_and_updates_the_target() {
        let dir = tempfile::tempdir().unwrap();
        let current = dir.path().join("downloaded-update.exe");
        let target = dir.path().join(EXECUTABLE_NAME);
        std::fs::write(&current, b"new executable").unwrap();
        std::fs::write(&target, b"old executable").unwrap();
        replace_executable(&current, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new executable");
        assert_eq!(std::fs::read(&current).unwrap(), b"new executable");
    }
}
