#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(non_snake_case, dead_code)]

#[cfg(not(windows))]
fn main() {
    eprintln!("Photon Setup is only supported on Windows.");
}

#[cfg(windows)]
fn main() {
    win_setup::run();
}

#[cfg(windows)]
mod win_setup {
    use std::env;
    use std::ffi::OsStr;
    use std::fs;
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::ptr;
    use std::sync::Mutex;

    // Embedded production executable of Photon Viewer decoupled via OUT_DIR payload
    const VIEWER_EXE_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/photon_viewer_payload.bin"));

    type HWND = *mut std::ffi::c_void;
    type HDC = *mut std::ffi::c_void;
    type HFONT = *mut std::ffi::c_void;
    type HBRUSH = *mut std::ffi::c_void;
    type HPEN = *mut std::ffi::c_void;
    type LRESULT = isize;
    type UINT = u32;
    type WPARAM = usize;
    type LPARAM = isize;
    type BOOL = i32;
    type DWORD = u32;
    type LPCWSTR = *const u16;
    type HKEY = *mut std::ffi::c_void;

    #[repr(C)]
    struct WNDCLASSW {
        style: UINT,
        lpfnWndProc: unsafe extern "system" fn(HWND, UINT, WPARAM, LPARAM) -> LRESULT,
        cbClsExtra: i32,
        cbWndExtra: i32,
        hInstance: *mut std::ffi::c_void,
        hIcon: *mut std::ffi::c_void,
        hCursor: *mut std::ffi::c_void,
        hbrBackground: HBRUSH,
        lpszMenuName: LPCWSTR,
        lpszClassName: LPCWSTR,
    }

    #[repr(C)]
    struct RECT {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    struct PAINTSTRUCT {
        hdc: HDC,
        fErase: BOOL,
        rcPaint: RECT,
        fRestore: BOOL,
        fIncUpdate: BOOL,
        rgbReserved: [u8; 32],
    }

    #[repr(C)]
    struct POINT {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct MSG {
        hwnd: HWND,
        message: UINT,
        wParam: WPARAM,
        lParam: LPARAM,
        time: DWORD,
        pt: POINT,
    }

    const WS_OVERLAPPED: DWORD = 0x00000000;
    const WS_CAPTION: DWORD = 0x00C00000;
    const WS_SYSMENU: DWORD = 0x00080000;
    const WS_MINIMIZEBOX: DWORD = 0x00020000;
    const WS_VISIBLE: DWORD = 0x10000000;
    const WS_CHILD: DWORD = 0x40000000;
    const WS_TABSTOP: DWORD = 0x00010000;

    const BS_GROUPBOX: DWORD = 0x00000007;
    const BS_AUTOCHECKBOX: DWORD = 0x00000003;
    const BS_DEFPUSHBUTTON: DWORD = 0x00000001;
    const BS_PUSHBUTTON: DWORD = 0x00000000;

    const BM_GETCHECK: UINT = 0x00F0;
    const BM_SETCHECK: UINT = 0x00F1;
    const BST_CHECKED: WPARAM = 1;
    const BST_UNCHECKED: WPARAM = 0;

    const WM_DESTROY: UINT = 0x0002;
    const WM_PAINT: UINT = 0x000F;
    const WM_COMMAND: UINT = 0x0111;
    const WM_SETFONT: UINT = 0x0030;
    const WM_CTLCOLORSTATIC: UINT = 0x0138;

    const COLOR_BTNFACE: i32 = 15;
    const COLOR_WINDOW: i32 = 5;

    const CS_HREDRAW: UINT = 0x0002;
    const CS_VREDRAW: UINT = 0x0001;
    const IDC_ARROW: LPCWSTR = 32512 as LPCWSTR;

    const MB_OK: UINT = 0x00000000;
    const MB_YESNO: UINT = 0x00000004;
    const MB_ICONINFORMATION: UINT = 0x00000040;
    const MB_ICONERROR: UINT = 0x00000010;
    const IDYES: i32 = 6;

    const HKEY_CURRENT_USER: HKEY = 0x80000001usize as HKEY;
    const KEY_WRITE: DWORD = 0x00020006;
    const REG_SZ: DWORD = 1;

    #[link(name = "user32")]
    #[link(name = "gdi32")]
    #[link(name = "shell32")]
    #[link(name = "advapi32")]
    extern "system" {
        fn RegisterClassW(lpWndClass: *const WNDCLASSW) -> u16;
        fn CreateWindowExW(
            dwExStyle: DWORD,
            lpClassName: LPCWSTR,
            lpWindowName: LPCWSTR,
            dwStyle: DWORD,
            X: i32,
            Y: i32,
            nWidth: i32,
            nHeight: i32,
            hWndParent: HWND,
            hMenu: *mut std::ffi::c_void,
            hInstance: *mut std::ffi::c_void,
            lpParam: *mut std::ffi::c_void,
        ) -> HWND;
        fn ShowWindow(hWnd: HWND, nCmdShow: i32) -> BOOL;
        fn UpdateWindow(hWnd: HWND) -> BOOL;
        fn GetMessageW(lpMsg: *mut MSG, hWnd: HWND, wMsgFilterMin: UINT, wMsgFilterMax: UINT) -> BOOL;
        fn TranslateMessage(lpMsg: *const MSG) -> BOOL;
        fn DispatchMessageW(lpMsg: *const MSG) -> LRESULT;
        fn PostQuitMessage(nExitCode: i32);
        fn DefWindowProcW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
        fn SendMessageW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
        fn BeginPaint(hWnd: HWND, lpPaint: *mut PAINTSTRUCT) -> HDC;
        fn EndPaint(hWnd: HWND, lpPaint: *const PAINTSTRUCT) -> BOOL;
        fn FillRect(hDC: HDC, lprc: *const RECT, hbr: HBRUSH) -> i32;
        fn GetSysColorBrush(nIndex: i32) -> HBRUSH;
        fn GetSystemMetrics(nIndex: i32) -> i32;
        fn GetModuleHandleW(lpModuleName: LPCWSTR) -> *mut std::ffi::c_void;
        fn LoadIconW(hInstance: *mut std::ffi::c_void, lpIconName: LPCWSTR) -> *mut std::ffi::c_void;
        fn LoadCursorW(hInstance: *mut std::ffi::c_void, lpCursorName: LPCWSTR) -> *mut std::ffi::c_void;
        fn MessageBoxW(hWnd: HWND, lpText: LPCWSTR, lpCaption: LPCWSTR, uType: UINT) -> i32;
        fn CreateFontW(
            cHeight: i32,
            cWidth: i32,
            cEscapement: i32,
            cOrientation: i32,
            cWeight: i32,
            bItalic: DWORD,
            bUnderline: DWORD,
            bStrikeOut: DWORD,
            iCharSet: DWORD,
            iOutPrecision: DWORD,
            iClipPrecision: DWORD,
            iQuality: DWORD,
            iPitchAndFamily: DWORD,
            pszFaceName: LPCWSTR,
        ) -> HFONT;
        fn SetBkMode(hdc: HDC, mode: i32) -> i32;
        fn SetTextColor(hdc: HDC, color: DWORD) -> DWORD;
        fn TextOutW(hdc: HDC, x: i32, y: i32, lpString: LPCWSTR, c: i32) -> BOOL;
        fn CreateSolidBrush(color: DWORD) -> HBRUSH;
        fn CreatePen(iStyle: i32, cWidth: i32, color: DWORD) -> HPEN;
        fn SelectObject(hdc: HDC, h: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        fn DeleteObject(ho: *mut std::ffi::c_void) -> BOOL;
        fn Ellipse(hdc: HDC, left: i32, top: i32, right: i32, bottom: i32) -> BOOL;
        fn MoveToEx(hdc: HDC, x: i32, y: i32, lppt: *mut POINT) -> BOOL;
        fn LineTo(hdc: HDC, x: i32, y: i32) -> BOOL;

        fn RegCreateKeyExW(
            hKey: HKEY,
            lpSubKey: LPCWSTR,
            Reserved: DWORD,
            lpClass: LPCWSTR,
            dwOptions: DWORD,
            samDesired: DWORD,
            lpSecurityAttributes: *mut std::ffi::c_void,
            phkResult: *mut HKEY,
            lpdwDisposition: *mut DWORD,
        ) -> i32;
        fn RegSetValueExW(
            hKey: HKEY,
            lpValueName: LPCWSTR,
            Reserved: DWORD,
            dwType: DWORD,
            lpData: *const u8,
            cbData: DWORD,
        ) -> i32;
        fn RegCloseKey(hKey: HKEY) -> i32;
        fn SHChangeNotify(wEventId: i32, uFlags: u32, dwItem1: *const std::ffi::c_void, dwItem2: *const std::ffi::c_void);
    }

    fn to_wstr(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(Some(0)).collect()
    }

    // Checkbox format definitions: (ControlID, Label, Extensions)
    struct FormatItem {
        id: usize,
        label: &'static str,
        exts: &'static [&'static str],
    }

    static FORMATS: &[FormatItem] = &[
        // Column 1: Standard Images
        FormatItem { id: 101, label: "JPG / JPEG", exts: &["jpg", "jpeg"] },
        FormatItem { id: 102, label: "PNG", exts: &["png"] },
        FormatItem { id: 103, label: "WebP", exts: &["webp"] },
        FormatItem { id: 104, label: "GIF (Anim)", exts: &["gif"] },
        FormatItem { id: 105, label: "BMP", exts: &["bmp"] },
        FormatItem { id: 106, label: "TIFF", exts: &["tif", "tiff"] },

        // Column 2: Camera RAW
        FormatItem { id: 107, label: "ARW (Sony)", exts: &["arw"] },
        FormatItem { id: 108, label: "CR2 / CR3", exts: &["cr2", "cr3"] },
        FormatItem { id: 109, label: "NEF (Nikon)", exts: &["nef"] },
        FormatItem { id: 110, label: "RAF (Fuji)", exts: &["raf"] },
        FormatItem { id: 111, label: "DNG (RAW)", exts: &["dng"] },
        FormatItem { id: 112, label: "ORF / RW2", exts: &["orf", "rw2"] },

        // Column 3: Specialty & Creative
        FormatItem { id: 113, label: "PSD (Adobe)", exts: &["psd"] },
        FormatItem { id: 114, label: "SVG (Vector)", exts: &["svg"] },
        FormatItem { id: 115, label: "HDR / EXR", exts: &["hdr", "exr"] },
        FormatItem { id: 116, label: "ICO / CUR", exts: &["ico", "cur"] },
        FormatItem { id: 117, label: "TGA", exts: &["tga"] },
        FormatItem { id: 118, label: "PNM / QOI", exts: &["pnm", "ppm", "qoi"] },
    ];

    const ID_TOGGLE_ALL: usize = 201;
    const ID_DESKTOP_SHORTCUT: usize = 202;
    const ID_START_SHORTCUT: usize = 203;
    const ID_SHELL_INTEGRATION: usize = 204;
    const ID_BTN_INSTALL: usize = 301;
    const ID_BTN_CANCEL: usize = 302;

    struct InstallerState {
        checkbox_hwnds: Vec<HWND>,
        desktop_hwnd: HWND,
        start_hwnd: HWND,
        shell_hwnd: HWND,
        all_checked: bool,
    }

    unsafe impl Send for InstallerState {}
    unsafe impl Sync for InstallerState {}

    static STATE: Mutex<InstallerState> = Mutex::new(InstallerState {
        checkbox_hwnds: Vec::new(),
        desktop_hwnd: ptr::null_mut(),
        start_hwnd: ptr::null_mut(),
        shell_hwnd: ptr::null_mut(),
        all_checked: true,
    });

    fn set_registry_value(hkey_root: HKEY, subkey: &str, value_name: &str, data: &str) {
        let subkey_w = to_wstr(subkey);
        let valname_w = to_wstr(value_name);
        let data_w = to_wstr(data);
        let mut hkey: HKEY = ptr::null_mut();

        unsafe {
            let res = RegCreateKeyExW(
                hkey_root,
                subkey_w.as_ptr(),
                0,
                ptr::null_mut(),
                0,
                KEY_WRITE,
                ptr::null_mut(),
                &mut hkey,
                ptr::null_mut(),
            );
            if res == 0 && !hkey.is_null() {
                let bytes_len = (data_w.len() * 2) as DWORD;
                RegSetValueExW(
                    hkey,
                    valname_w.as_ptr(),
                    0,
                    REG_SZ,
                    data_w.as_ptr() as *const u8,
                    bytes_len,
                );
                RegCloseKey(hkey);
            }
        }
    }

    fn create_shortcut(target_exe: &Path, shortcut_path: &Path) {
        let script = format!(
            "$ws = New-Object -ComObject WScript.Shell; $s = $ws.CreateShortcut('{}'); $s.TargetPath = '{}'; $s.Description = 'Photon Viewer'; $s.Save();",
            shortcut_path.to_string_lossy().replace('\'', "''"),
            target_exe.to_string_lossy().replace('\'', "''")
        );

        let _ = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output();
    }

    fn perform_install(hwnd_parent: HWND) {
        if VIEWER_EXE_BYTES.is_empty() {
            unsafe {
                MessageBoxW(
                    hwnd_parent,
                    to_wstr("Installer payload not bundled. Run 'cargo build --release' first to compile photon_viewer.exe.").as_ptr(),
                    to_wstr("Install Error").as_ptr(),
                    MB_OK | MB_ICONERROR,
                );
            }
            return;
        }

        let local_app_data = match env::var("LOCALAPPDATA") {
            Ok(dir) => PathBuf::from(dir),
            Err(_) => {
                unsafe {
                    MessageBoxW(
                        hwnd_parent,
                        to_wstr("Failed to locate %LOCALAPPDATA% directory.").as_ptr(),
                        to_wstr("Install Error").as_ptr(),
                        MB_OK | MB_ICONERROR,
                    );
                }
                return;
            }
        };

        let install_dir = local_app_data.join("Programs").join("PhotonViewer");
        if let Err(e) = fs::create_dir_all(&install_dir) {
            unsafe {
                let msg = format!("Failed to create install directory: {}", e);
                MessageBoxW(
                    hwnd_parent,
                    to_wstr(&msg).as_ptr(),
                    to_wstr("Install Error").as_ptr(),
                    MB_OK | MB_ICONERROR,
                );
            }
            return;
        }

        let exe_path = install_dir.join("photon_viewer.exe");

        // Terminate any old instance if open
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "photon_viewer.exe"])
            .output();
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Write binary
        if let Err(e) = fs::write(&exe_path, VIEWER_EXE_BYTES) {
            unsafe {
                let msg = format!("Failed to write photon_viewer.exe: {}\nPlease close any running instances.", e);
                MessageBoxW(
                    hwnd_parent,
                    to_wstr(&msg).as_ptr(),
                    to_wstr("Install Error").as_ptr(),
                    MB_OK | MB_ICONERROR,
                );
            }
            return;
        }

        let exe_str = exe_path.to_string_lossy().to_string();
        let command_str = format!("\"{}\" \"%1\"", exe_str);
        let icon_str = format!("\"{}\",0", exe_str);

        // 1. Register ProgID in HKCU\Software\Classes
        set_registry_value(HKEY_CURRENT_USER, r"Software\Classes\PhotonViewer.Image", "", "Photon Image File");
        set_registry_value(HKEY_CURRENT_USER, r"Software\Classes\PhotonViewer.Image\DefaultIcon", "", &icon_str);
        set_registry_value(HKEY_CURRENT_USER, r"Software\Classes\PhotonViewer.Image\shell\open\command", "", &command_str);

        // 2. Register Application in HKCU\Software\Classes\Applications\photon_viewer.exe
        set_registry_value(HKEY_CURRENT_USER, r"Software\Classes\Applications\photon_viewer.exe", "FriendlyAppName", "Photon Viewer");
        set_registry_value(HKEY_CURRENT_USER, r"Software\Classes\Applications\photon_viewer.exe\DefaultIcon", "", &icon_str);
        set_registry_value(HKEY_CURRENT_USER, r"Software\Classes\Applications\photon_viewer.exe\shell\open\command", "", &command_str);

        // 3. Register Selected File Associations
        let checkbox_hwnds = {
            let state = STATE.lock().unwrap();
            state.checkbox_hwnds.clone()
        };

        unsafe {
            for (i, item) in FORMATS.iter().enumerate() {
                if i < checkbox_hwnds.len() {
                    let state = SendMessageW(checkbox_hwnds[i], BM_GETCHECK, 0, 0);
                    if state == BST_CHECKED as isize {
                        for &ext in item.exts {
                            let dot_ext = format!(".{}", ext);
                            let subkey_ext = format!(r"Software\Classes\{}", dot_ext);
                            let subkey_openwith = format!(r"Software\Classes\{}\OpenWithProgids", dot_ext);
                            let subkey_supported = format!(r"Software\Classes\Applications\photon_viewer.exe\SupportedTypes");

                            set_registry_value(HKEY_CURRENT_USER, &subkey_ext, "", "PhotonViewer.Image");
                            set_registry_value(HKEY_CURRENT_USER, &subkey_openwith, "PhotonViewer.Image", "");
                            set_registry_value(HKEY_CURRENT_USER, &subkey_supported, &dot_ext, "");
                        }
                    }
                }
            }

            let (desktop_hwnd, start_hwnd, shell_hwnd) = {
                let state = STATE.lock().unwrap();
                (state.desktop_hwnd, state.start_hwnd, state.shell_hwnd)
            };

            // 4. Shell Context Menu Integration
            let shell_checked = SendMessageW(shell_hwnd, BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
            if shell_checked {
                set_registry_value(HKEY_CURRENT_USER, r"Software\Classes\*\shell\PhotonViewer", "", "Open with Photon Viewer");
                set_registry_value(HKEY_CURRENT_USER, r"Software\Classes\*\shell\PhotonViewer", "Icon", &icon_str);
                set_registry_value(HKEY_CURRENT_USER, r"Software\Classes\*\shell\PhotonViewer\command", "", &command_str);
            }

            // 5. Desktop Shortcut
            let desktop_checked = SendMessageW(desktop_hwnd, BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
            if desktop_checked {
                if let Ok(user_profile) = env::var("USERPROFILE") {
                    let desktop_path = PathBuf::from(user_profile).join("Desktop").join("Photon Viewer.lnk");
                    create_shortcut(&exe_path, &desktop_path);
                }
            }

            // 6. Start Menu Shortcut
            let start_checked = SendMessageW(start_hwnd, BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
            if start_checked {
                if let Ok(app_data) = env::var("APPDATA") {
                    let start_path = PathBuf::from(app_data)
                        .join("Microsoft")
                        .join("Windows")
                        .join("Start Menu")
                        .join("Programs")
                        .join("Photon Viewer.lnk");
                    create_shortcut(&exe_path, &start_path);
                }
            }

            // 7. Tell Windows Explorer shell to refresh icons immediately!
            SHChangeNotify(0x08000000, 0, ptr::null(), ptr::null()); // SHCNE_ASSOCCHANGED

            // 8. Completed Dialog with launch prompt
            let success_msg = "Photon Viewer has been successfully installed!\n\nSelected formats are now associated and will open with instant GPU acceleration.\n\nWould you like to launch Photon Viewer now?";
            let res = MessageBoxW(
                hwnd_parent,
                to_wstr(success_msg).as_ptr(),
                to_wstr("Setup Complete").as_ptr(),
                MB_YESNO | MB_ICONINFORMATION,
            );

            if res == IDYES {
                let _ = Command::new(&exe_path).spawn();
            }

            PostQuitMessage(0);
        }
    }

    unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match msg {
            WM_PAINT => {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let hdc = BeginPaint(hwnd, &mut ps);

                // 1. Draw Vintage Header Banner (height: 60px)
                let header_brush = CreateSolidBrush(0x00181512); // Sleek vintage dark obsidian (RGB: 18, 21, 24)
                let header_rect = RECT { left: 0, top: 0, right: 600, bottom: 62 };
                FillRect(hdc, &header_rect, header_brush);
                DeleteObject(header_brush as *mut std::ffi::c_void);

                // Subtle divider line
                let line_pen = CreatePen(0, 1, 0x003A322C);
                let old_pen = SelectObject(hdc, line_pen as *mut std::ffi::c_void);
                MoveToEx(hdc, 0, 62, ptr::null_mut());
                LineTo(hdc, 600, 62);
                SelectObject(hdc, old_pen);
                DeleteObject(line_pen as *mut std::ffi::c_void);

                // 2. Draw Retro Photon Logo (Vintage Camera Aperture & Prism Ray)
                let ring_pen = CreatePen(0, 2, 0x00D09B28); // Amber gold outline
                let lens_brush = CreateSolidBrush(0x002A241F); // Dark optical glass
                let old_p = SelectObject(hdc, ring_pen as *mut std::ffi::c_void);
                let old_b = SelectObject(hdc, lens_brush as *mut std::ffi::c_void);
                Ellipse(hdc, 16, 11, 56, 51); // 40x40 circle

                // Inner aperture blades & photon light pulse
                let iris_pen = CreatePen(0, 1, 0x00E8C040);
                SelectObject(hdc, iris_pen as *mut std::ffi::c_void);
                MoveToEx(hdc, 22, 21, ptr::null_mut()); LineTo(hdc, 50, 41);
                MoveToEx(hdc, 50, 21, ptr::null_mut()); LineTo(hdc, 22, 41);
                MoveToEx(hdc, 36, 12, ptr::null_mut()); LineTo(hdc, 36, 50);

                // Lens glass reflection highlight (cyan photon ray)
                let ray_pen = CreatePen(0, 2, 0x00E0E060); // Electric cyan beam
                SelectObject(hdc, ray_pen as *mut std::ffi::c_void);
                MoveToEx(hdc, 26, 18, ptr::null_mut()); LineTo(hdc, 46, 38);

                SelectObject(hdc, old_p);
                SelectObject(hdc, old_b);
                DeleteObject(ring_pen as *mut std::ffi::c_void);
                DeleteObject(lens_brush as *mut std::ffi::c_void);
                DeleteObject(iris_pen as *mut std::ffi::c_void);
                DeleteObject(ray_pen as *mut std::ffi::c_void);

                // 3. Header Typography
                SetBkMode(hdc, 1); // TRANSPARENT
                let title_font = CreateFontW(20, 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 5, 0, to_wstr("Segoe UI").as_ptr());
                let sub_font = CreateFontW(14, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 5, 0, to_wstr("Segoe UI").as_ptr());

                let old_font = SelectObject(hdc, title_font as *mut std::ffi::c_void);
                SetTextColor(hdc, 0x00FFFFFF); // Pure white
                let title = to_wstr("Photon Viewer Setup");
                TextOutW(hdc, 68, 14, title.as_ptr(), (title.len() - 1) as i32);

                SelectObject(hdc, sub_font as *mut std::ffi::c_void);
                SetTextColor(hdc, 0x00B0B8C0); // Soft grey
                let subtitle = to_wstr("Ultra-Fast GPU Image & Camera RAW Viewer");
                TextOutW(hdc, 68, 36, subtitle.as_ptr(), (subtitle.len() - 1) as i32);

                SelectObject(hdc, old_font);
                DeleteObject(title_font as *mut std::ffi::c_void);
                DeleteObject(sub_font as *mut std::ffi::c_void);

                EndPaint(hwnd, &ps);
                0
            }
            WM_CTLCOLORSTATIC => {
                GetSysColorBrush(COLOR_BTNFACE) as LRESULT
            }
            WM_COMMAND => {
                let ctrl_id = (wparam & 0xFFFF) as usize;
                if ctrl_id == ID_TOGGLE_ALL {
                    let mut state = STATE.lock().unwrap();
                    state.all_checked = !state.all_checked;
                    let new_state = if state.all_checked { BST_CHECKED } else { BST_UNCHECKED };
                    for &chk_hwnd in state.checkbox_hwnds.iter() {
                        SendMessageW(chk_hwnd, BM_SETCHECK, new_state, 0);
                    }
                } else if ctrl_id == ID_BTN_INSTALL {
                    perform_install(hwnd);
                } else if ctrl_id == ID_BTN_CANCEL {
                    PostQuitMessage(0);
                }
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    pub fn run() {
        unsafe {
            let hinst = GetModuleHandleW(ptr::null());
            let hicon = LoadIconW(hinst, 1 as LPCWSTR);

            let class_name = to_wstr("PhotonViewerSetupClass");
            let wnd_class = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: wnd_proc,
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinst,
                hIcon: hicon,
                hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
                hbrBackground: GetSysColorBrush(COLOR_BTNFACE),
                lpszMenuName: ptr::null(),
                lpszClassName: class_name.as_ptr(),
            };

            RegisterClassW(&wnd_class);

            // Center window on primary monitor
            let win_w = 560;
            let win_h = 570;
            let screen_w = GetSystemMetrics(0); // SM_CXSCREEN
            let screen_h = GetSystemMetrics(1); // SM_CYSCREEN
            let pos_x = (screen_w - win_w) / 2;
            let pos_y = (screen_h - win_h) / 2;

            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                to_wstr("Photon Viewer Setup - File Associations & Integration").as_ptr(),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_VISIBLE,
                pos_x,
                pos_y,
                win_w,
                win_h,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            );

            if hwnd.is_null() {
                return;
            }

            // Standard Win32 dialog font (Segoe UI, 9pt)
            let gui_font = CreateFontW(15, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 5, 0, to_wstr("Segoe UI").as_ptr());

            let create_ctrl = |class: &str, text: &str, style: DWORD, x: i32, y: i32, w: i32, h: i32, id: usize| -> HWND {
                let ctrl = CreateWindowExW(
                    0,
                    to_wstr(class).as_ptr(),
                    to_wstr(text).as_ptr(),
                    WS_CHILD | WS_VISIBLE | style,
                    x,
                    y,
                    w,
                    h,
                    hwnd,
                    id as *mut std::ffi::c_void,
                    ptr::null_mut(),
                    ptr::null_mut(),
                );
                SendMessageW(ctrl, WM_SETFONT, gui_font as WPARAM, 1);
                ctrl
            };

            // 1. GroupBox: "Associate Photon Viewer with"
            create_ctrl("BUTTON", "Associate Photon Viewer with", BS_GROUPBOX, 14, 72, 330, 240, 0);

            // 3 Columns of Checkboxes inside the GroupBox
            let col_x = [26, 134, 240];
            let row_start_y = 96;
            let row_spacing = 24;

            {
                let mut state = STATE.lock().unwrap();
                state.checkbox_hwnds.clear();
                for (i, item) in FORMATS.iter().enumerate() {
                    let col = i / 6;
                    let row = i % 6;
                    let x = col_x[col];
                    let y = row_start_y + (row as i32 * row_spacing);

                    let chk = create_ctrl("BUTTON", item.label, BS_AUTOCHECKBOX | WS_TABSTOP, x, y, 102, 20, item.id);
                    SendMessageW(chk, BM_SETCHECK, BST_CHECKED, 0); // Checked by default
                    state.checkbox_hwnds.push(chk);
                }
            }

            // Toggle all button
            create_ctrl("BUTTON", "Toggle all", BS_PUSHBUTTON | WS_TABSTOP, 26, 272, 100, 25, ID_TOGGLE_ALL);

            // 2. Right GroupBox 1: "Interface"
            create_ctrl("BUTTON", "Interface", BS_GROUPBOX, 356, 72, 174, 90, 0);
            let desktop_hwnd = create_ctrl("BUTTON", "Add to Desktop", BS_AUTOCHECKBOX | WS_TABSTOP, 368, 96, 150, 20, ID_DESKTOP_SHORTCUT);
            let start_hwnd = create_ctrl("BUTTON", "Add to Start Menu", BS_AUTOCHECKBOX | WS_TABSTOP, 368, 124, 150, 20, ID_START_SHORTCUT);
            SendMessageW(desktop_hwnd, BM_SETCHECK, BST_CHECKED, 0);
            SendMessageW(start_hwnd, BM_SETCHECK, BST_CHECKED, 0);

            // 3. Right GroupBox 2: "Shell integration"
            create_ctrl("BUTTON", "Shell integration", BS_GROUPBOX, 356, 172, 174, 140, 0);
            let shell_hwnd = create_ctrl("BUTTON", "Integrate into shell", BS_AUTOCHECKBOX | WS_TABSTOP, 368, 196, 150, 20, ID_SHELL_INTEGRATION);
            let chk_cascaded = create_ctrl("BUTTON", "Fast right-click", BS_AUTOCHECKBOX | WS_TABSTOP, 368, 224, 150, 20, 0);
            let chk_icons = create_ctrl("BUTTON", "Icons in menus", BS_AUTOCHECKBOX | WS_TABSTOP, 368, 252, 150, 20, 0);
            SendMessageW(shell_hwnd, BM_SETCHECK, BST_CHECKED, 0);
            SendMessageW(chk_cascaded, BM_SETCHECK, BST_CHECKED, 0);
            SendMessageW(chk_icons, BM_SETCHECK, BST_CHECKED, 0);

            {
                let mut state = STATE.lock().unwrap();
                state.desktop_hwnd = desktop_hwnd;
                state.start_hwnd = start_hwnd;
                state.shell_hwnd = shell_hwnd;
            }

            // 4. Informational Text Box
            let desc_text = "These options control Photon Viewer integration into Windows. The first group of options allows to choose image and camera RAW formats to handle by Photon Viewer. The second group selects places to add links to the executable. Shell integration provides a handy 'Open with Photon Viewer' item in context menus.\n\nClick 'Install' to register file associations and finish setup.";
            create_ctrl("STATIC", desc_text, 0, 14, 324, 516, 120, 0);

            // Subtle lower divider
            let _lower_line = create_ctrl("STATIC", "", 0x00000010, 14, 460, 516, 2, 0); // SS_ETCHEDHORZ

            // 5. Action Buttons
            create_ctrl("BUTTON", "Install", BS_DEFPUSHBUTTON | WS_TABSTOP, 290, 480, 114, 32, ID_BTN_INSTALL);
            create_ctrl("BUTTON", "Cancel", BS_PUSHBUTTON | WS_TABSTOP, 416, 480, 114, 32, ID_BTN_CANCEL);

            ShowWindow(hwnd, 1); // SW_SHOWNORMAL
            UpdateWindow(hwnd);

            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}
