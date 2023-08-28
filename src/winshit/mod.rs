use egui::Key;
use epaint::image::ColorImage;
use epaint::Color32;
use log::{debug, error, trace, warn};
use std::error::Error;
use std::fmt::Debug;
use std::ptr::NonNull;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::CreateCompatibleDC;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, SetFocus, UnregisterHotKey, MOD_ALT, MOD_CONTROL,
};
use windows::Win32::UI::WindowsAndMessaging::*;

pub struct WindowInfo {
    pub handle: HWND,
    pub title: String,
    pub icon: Option<ColorImage>,
    pub icon_size: Option<(usize, usize)>,
}

impl Debug for WindowInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowInfo")
            .field("handle", &self.handle)
            .field("title", &self.title)
            .field(
                "icon",
                &self
                    .icon
                    .as_ref()
                    .map(|img| format!("ColorImage({}x{})", img.width(), img.height())),
            )
            .field("icon_size", &self.icon_size)
            .finish()
    }
}

pub unsafe extern "system" fn enum_window_proc(handle: HWND, param: LPARAM) -> BOOL {
    let style = GetWindowLongW(handle, GWL_STYLE) as u32;
    let ex_style = GetWindowLongW(handle, GWL_EXSTYLE) as u32;

    if (style & WS_VISIBLE.0) != 0 && (ex_style & WS_EX_TOOLWINDOW.0) == 0 {
        let buf = &mut [0u16; 1024];
        let len = GetWindowTextW(handle, buf);
        let title = String::from_utf16_lossy(&buf[..len as usize]);

        let mut class_buf = [0u16; 256];
        let class_len = GetClassNameW(handle, &mut class_buf);
        let class_name = String::from_utf16_lossy(&class_buf[..class_len as usize]);

        if title.is_empty() || class_name == "Windows.UI.Core.CoreWindow" {
            return TRUE;
        }

        unsafe {
            let windows_ptr = param.0 as *mut Vec<WindowInfo>;
            let windows = NonNull::new(windows_ptr).expect("Null pointer").as_mut();

            let (icon_data, icon_size) = match get_window_icon_data(handle) {
                Ok((data, size)) => {
                    let image = convert_to_color_image(data, size.0, size.1);
                    (Some(image), Some(size))
                }
                Err(e) => {
                    warn!("Failed to get icon for window '{}': {:?}", title, e);
                    (
                        Some(ColorImage {
                            size: [32, 32],
                            pixels: vec![Color32::TRANSPARENT; 32 * 32],
                        }),
                        Some((32 as usize, 32 as usize)),
                    )
                }
            };

            windows.push(WindowInfo {
                handle,
                title: title.clone(),
                icon: icon_data,
                icon_size,
            });
        }
    }
    TRUE
}


pub unsafe fn focus_window(handle: HWND) {
    if IsIconic(handle) != FALSE {
        if ShowWindow(handle, SW_NORMAL) == FALSE {
            let error_code = GetLastError();
            error!("ShowWindow failed with error code: {:?}", error_code);
        }
    }

    //this is marked 'not intended for general use'
    SwitchToThisWindow(handle, TRUE);
}

#[derive(Debug)]
pub enum WindowIconError {
    NoIcon,
    CreateDIBSectionFailed,
    GetDIBitsFailed,
    GetIconInfoFailed,
}

pub unsafe fn get_window_icon_data(
    handle: HWND,
) -> Result<(Vec<u8>, (usize, usize)), WindowIconError> {
    let mut hicon = HICON(SendMessageW(handle, WM_GETICON, WPARAM(ICON_BIG as usize), LPARAM(0)).0);
    if hicon.0 == 0 {
        trace!("No big icon from message, trying class ptr");
        hicon = HICON(GetClassLongPtrW(handle, GCLP_HICON) as isize);
    }
    if hicon.0 == 0 {
        trace!("No big icon from class ptr, trying small icon");
        hicon = HICON(SendMessageW(handle, WM_GETICON, WPARAM(ICON_SMALL as usize), LPARAM(0)).0);
    }
    if hicon.0 == 0 {
        trace!("No small icon from message, trying class ptr");
        hicon = HICON(GetClassLongPtrW(handle, GCLP_HICONSM) as isize);
    }

    if hicon.0 == 0 {
        return Err(WindowIconError::NoIcon);
    }

    let mut icon_info = ICONINFO::default();
    if GetIconInfo(hicon, &mut icon_info) == FALSE {
        return Err(WindowIconError::GetIconInfoFailed);
    }

    let icon_width = (icon_info.xHotspot * 2) as usize;
    let icon_height = (icon_info.yHotspot * 2) as usize;
    trace!("{}: icon size: {}x{}", handle.0, icon_width, icon_height);

    let hdc = CreateCompatibleDC(HDC(0));
    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: icon_width as i32,
            biHeight: -(icon_height as i32), // Negative to indicate top-down DIB
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0 as u32,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut icon_data: Vec<u8> = Vec::with_capacity(icon_width * icon_height * 4);
    trace!("{}: icon_data capacity: {}", handle.0, icon_data.capacity());
    let mut bits_ptr: *mut ::core::ffi::c_void = std::ptr::null_mut();
    let hbitmap = match CreateDIBSection(hdc, &bmi, DIB_RGB_COLORS, &mut bits_ptr, HANDLE(0), 0) {
        Ok(hbitmap) => hbitmap,
        Err(_) => return Err(WindowIconError::CreateDIBSectionFailed),
    };

    let holdbitmap = SelectObject(hdc, hbitmap);
    DrawIconEx(
        hdc,
        0,
        0,
        hicon,
        icon_width as i32,
        icon_height as i32,
        0,
        HBRUSH(0),
        DI_NORMAL,
    );

    if GetDIBits(
        hdc,
        icon_info.hbmColor,
        0,
        icon_height as u32,
        Some(bits_ptr),
        &mut bmi,
        DIB_RGB_COLORS,
    ) == 0
    {
        let error_code = GetLastError();
        error!("GetDIBits failed with error code {:?}", error_code);
        return Err(WindowIconError::GetDIBitsFailed);
    }

    let raw_data = std::slice::from_raw_parts(bits_ptr as *const u8, icon_width * icon_height * 4);
    icon_data.extend_from_slice(raw_data);

    SelectObject(hdc, holdbitmap);
    DeleteObject(hbitmap);
    DeleteDC(hdc);

    Ok((icon_data, (icon_width, icon_height)))
}

fn _save_icon_to_bin(data: Vec<u8>, name: String) -> std::io::Result<()> {
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;
    trace!("Saving icon to bin file '{}_icon.bin'", name);

    // Create a sanitized filename from the window title
    let sanitized_title = name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ')
        .collect::<String>();
    let filename = format!("{}_icon.bin", sanitized_title);

    // Ensure the filename is valid and doesn't contain any restricted characters
    if Path::new(&filename).is_file() {
        trace!("File '{}' already exists", filename);
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "File already exists",
        ));
    }

    // Write the icon data to the file
    let mut file = File::create(&filename)?;
    file.write_all(&data)?;

    Ok(())
}

fn convert_to_color_image(data: Vec<u8>, width: usize, height: usize) -> ColorImage {
    let mut pixels = Vec::with_capacity(width * height);

    for i in (0..data.len()).step_by(4) {
        let b = data[i];
        let g = data[i + 1];
        let r = data[i + 2];
        let a = data[i + 3];

        pixels.push(Color32::from_rgba_unmultiplied(r, g, b, a));
    }

    ColorImage {
        size: [width, height],
        pixels,
    }
}

pub const HOTKEY_ID: i32 = 1;
pub unsafe fn register_hotkey() -> Result<(), Box<dyn Error>> {
    let did_register = RegisterHotKey(HWND(0), HOTKEY_ID, MOD_CONTROL | MOD_ALT, 0x20);
    if did_register == FALSE {
        return Err(Box::new(std::io::Error::last_os_error()));
    } else {
        return Ok(());
    }
}

pub unsafe fn unregister_hotkey() -> Result<(), Box<dyn Error>> {
    let did_unregister = UnregisterHotKey(HWND(0), HOTKEY_ID);
    if did_unregister == FALSE {
        return Err(Box::new(std::io::Error::last_os_error()));
    } else {
        return Ok(());
    }
}
