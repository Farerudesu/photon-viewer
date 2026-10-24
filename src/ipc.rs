#[cfg(windows)]
mod win_ipc {
    use std::ffi::c_void;
    use std::io::{BufRead, BufReader, Write};
    use std::os::windows::io::{FromRawHandle, IntoRawHandle};
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
                    // CRITICAL: Reclaim ownership so File::drop does not call CloseHandle(handle)
                    // before DisconnectNamedPipe runs, preventing double-close and handle-recycling races.
                    let _ = reader.into_inner().into_raw_handle();
                    unsafe {
                        DisconnectNamedPipe(handle);
                        CloseHandle(handle);
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

#[cfg(unix)]
mod unix_ipc {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::{Path, PathBuf};
    use std::thread;
    use winit::event_loop::EventLoopProxy;

    fn get_socket_path() -> PathBuf {
        if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
            PathBuf::from(runtime_dir).join("photon_viewer.sock")
        } else {
            std::env::temp_dir().join("photon_viewer.sock")
        }
    }

    pub fn try_send_to_existing_instance(path: Option<&Path>) -> bool {
        let sock_path = get_socket_path();
        if let Ok(mut stream) = UnixStream::connect(&sock_path) {
            let msg = path.map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
            let _ = writeln!(stream, "{}", msg);
            let _ = stream.flush();
            return true;
        }
        if sock_path.exists() {
            let _ = std::fs::remove_file(&sock_path);
        }
        false
    }

    pub fn start_ipc_server(proxy: EventLoopProxy<PathBuf>) {
        let sock_path = get_socket_path();
        if sock_path.exists() {
            let _ = std::fs::remove_file(&sock_path);
        }

        thread::spawn(move || {
            let listener = match UnixListener::bind(&sock_path) {
                Ok(l) => l,
                Err(_) => return,
            };

            for stream in listener.incoming() {
                if let Ok(stream) = stream {
                    let mut reader = BufReader::new(stream);
                    let mut line = String::new();
                    if reader.read_line(&mut line).is_ok() {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            let path = PathBuf::from(trimmed);
                            let _ = proxy.send_event(path);
                        }
                    }
                }
            }
        });
    }
}

#[cfg(not(any(windows, unix)))]
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

#[cfg(unix)]
pub use unix_ipc::*;

#[cfg(not(any(windows, unix)))]
pub use non_win_ipc::*;
