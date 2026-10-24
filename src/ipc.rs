#[cfg(windows)]
mod win_ipc {
    use std::ffi::c_void;
    use std::io::{BufRead, BufReader, Write};
    use std::os::windows::io::FromRawHandle;
    use std::path::{Path, PathBuf};
    use std::ptr;
    use std::thread;
    use winit::event_loop::EventLoopProxy;

    type HANDLE = *mut c_void;
    type BOOL = i32;
    type DWORD = u32;
    type LPCWSTR = *const u16;

    const PIPE_ACCESS_INBOUND: DWORD = 0x00000001;
    const PIPE_TYPE_BYTE: DWORD = 0x00000000;
    const PIPE_READMODE_BYTE: DWORD = 0x00000000;
    const PIPE_WAIT: DWORD = 0x00000000;
    const INVALID_HANDLE_VALUE: HANDLE = -1isize as HANDLE;

    extern "system" {
        fn CreateNamedPipeW(
            lpName: LPCWSTR,
            dwOpenMode: DWORD,
            dwPipeMode: DWORD,
            nMaxInstances: DWORD,
            nOutBufferSize: DWORD,
            nInBufferSize: DWORD,
            nDefaultTimeOut: DWORD,
            lpSecurityAttributes: *mut c_void,
        ) -> HANDLE;
        fn ConnectNamedPipe(hNamedPipe: HANDLE, lpOverlapped: *mut c_void) -> BOOL;
        fn DisconnectNamedPipe(hNamedPipe: HANDLE) -> BOOL;
        fn CloseHandle(hObject: HANDLE) -> BOOL;
        fn GetLastError() -> DWORD;
    }

    const PIPE_NAME: &str = r"\\.\pipe\photon_viewer_single_instance";

    pub fn try_send_to_existing_instance(path: Option<&Path>) -> bool {
        if let Ok(mut file) = std::fs::OpenOptions::new().write(true).open(PIPE_NAME) {
            let msg = path.map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
            let _ = writeln!(file, "{}", msg);
            let _ = file.flush();
            return true;
        }
        false
    }

    pub fn start_ipc_server(proxy: EventLoopProxy<PathBuf>) {
        thread::spawn(move || {
            let pipe_name_w: Vec<u16> = "\\\\.\\pipe\\photon_viewer_single_instance\0"
                .encode_utf16()
                .collect();

            loop {
                let handle = unsafe {
                    CreateNamedPipeW(
                        pipe_name_w.as_ptr(),
                        PIPE_ACCESS_INBOUND,
                        PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                        1,
                        0,
                        4096,
                        0,
                        ptr::null_mut(),
                    )
                };

                if handle == INVALID_HANDLE_VALUE {
                    thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }

                let connected = unsafe { ConnectNamedPipe(handle, ptr::null_mut()) };
                if connected != 0 || unsafe { GetLastError() } == 535 {
                    // 535 = ERROR_PIPE_CONNECTED
                    let file = unsafe { std::fs::File::from_raw_handle(handle) };
                    let mut reader = BufReader::new(file);
                    let mut line = String::new();
                    if reader.read_line(&mut line).is_ok() {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            let path = PathBuf::from(trimmed);
                            let _ = proxy.send_event(path);
                        }
                    }
                    unsafe {
                        DisconnectNamedPipe(handle);
                    }
                } else {
                    unsafe {
                        CloseHandle(handle);
                    }
                }
            }
        });
    }
}

#[cfg(not(windows))]
mod non_win_ipc {
    use std::path::{Path, PathBuf};
    use winit::event_loop::EventLoopProxy;

    pub fn try_send_to_existing_instance(_path: Option<&Path>) -> bool {
        false
    }

    pub fn start_ipc_server(_proxy: EventLoopProxy<PathBuf>) {}
}

#[cfg(windows)]
pub use win_ipc::*;

#[cfg(not(windows))]
pub use non_win_ipc::*;
