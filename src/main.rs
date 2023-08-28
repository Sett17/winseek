#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod winshit;
use std::sync::mpsc;
use std::thread;

use log::{debug, error, info, log_enabled};
use winshit::*;
mod app;
use app::*;

use eframe::egui;
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use eframe::IconData;

static mut IS_OPEN: bool = false;
fn create_frame() -> std::result::Result<(), eframe::Error> {
    if unsafe { IS_OPEN } {
        return Ok(());
    }

    let screen_width: i32;
    let mut windows: Vec<WindowInfo> = Vec::new();

    unsafe {
        EnumWindows(
            Some(enum_window_proc),
            LPARAM(std::ptr::addr_of_mut!(windows) as isize),
        );

        if log_enabled!(log::Level::Debug) {
            for window in &windows {
                debug!("{:?}", window);
            }
        }

        screen_width = GetSystemMetrics(SM_CXSCREEN);
    }

    let app_width = std::cmp::max(screen_width / 3, 400) as f32;
    let x_center = (screen_width as f32 - app_width) / 2.0;

    const LOGO_DATA: &'static [u8] = include_bytes!("../logo.png");
    let icon_data = IconData::try_from_png_bytes(LOGO_DATA).unwrap();
    let options = eframe::NativeOptions {
        decorated: false,
        transparent: true,
        initial_window_size: Some(egui::vec2(app_width, 340.0)),
        initial_window_pos: Some(egui::Pos2::new(x_center, 200.0)),
        icon_data: Some(icon_data),
        always_on_top: true,
        resizable: false,
        ..Default::default()
    };

    unsafe {
        IS_OPEN = true;
    }
    let res = eframe::run_native(
        "WinSeek",
        options,
        Box::new(|_cc| {
            Box::<MyApp>::new(MyApp {
                windows,
                ..Default::default()
            })
        }),
    );
    unsafe {
        IS_OPEN = false;
    }
    res
}

fn main() -> std::result::Result<(), eframe::Error> {
    pretty_env_logger::init();

    let (tx, rx) = mpsc::channel();

    thread::spawn(move || unsafe {
        match register_hotkey() {
            Ok(_) => {}
            Err(_) => {
                error!("Failed to register hotkey");
                return;
            }
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, HWND(0), 0, 0).into() {
            if msg.message == WM_HOTKEY {
                if msg.wParam.0 == HOTKEY_ID as usize {
                    debug!("Hotkey pressed");
                    tx.send(()).unwrap();
                }
            }
            PeekMessageW(&mut msg, HWND(0), 0, 0, PM_REMOVE);
        }
        match unregister_hotkey() {
            Ok(_) => {}
            Err(_) => {
                error!("Failed to unregister hotkey");
                return;
            }
        }
    });

    let mut app = systray::Application::new().unwrap();
    app.set_icon_from_file("logo.ico").unwrap();

    app.add_menu_item("Open WinSeek", move |_| {
        create_frame().unwrap();
        Ok::<_, systray::Error>(())
    })
    .unwrap();

    app.add_menu_item("Exit", |window| -> Result<(), systray::Error> {
        window.quit();
        std::process::exit(0);
    })
    .unwrap();

    while let Ok(_) = rx.recv() {
        create_frame().unwrap();
    }

    app.wait_for_message().unwrap();

    Ok(())
}
