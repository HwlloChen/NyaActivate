use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use std::sync::OnceLock;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::config::{self, Config, Level};

static CONFIG: OnceLock<Config> = OnceLock::new();
static WATERMARK_HWND: OnceLock<isize> = OnceLock::new();
static SCREEN_X: AtomicI32 = AtomicI32::new(0);
static SCREEN_Y: AtomicI32 = AtomicI32::new(0);
static DPI_SCALE_X: AtomicI32 = AtomicI32::new(96);
static DPI_SCALE_Y: AtomicI32 = AtomicI32::new(96);
static HUE_OFFSET: AtomicU32 = AtomicU32::new(0);

const WINDOW_CLASS: &str = "NyaActivateWatermark";
const WM_PROGMAN_INIT: u32 = 0x052C;
const TIMER_COLORFUL: usize = 1;
const TIMER_DESKTOP_LOWER: usize = 2;
const WW: i32 = 350;
const WH: i32 = 85;
const GAP: i32 = 10;

pub fn show(config: Config) {
    log::info!("正在启动水印窗口");
    let _ = CONFIG.set(config);

    unsafe {
        // Enable per-monitor DPI awareness for sharp text on high-DPI displays
        SetProcessDPIAware();

        let inst = GetModuleHandleW(std::ptr::null());

        // Get DPI and compute scale factor
        let sdc = GetDC(std::ptr::null_mut());
        let dpi_x = GetDeviceCaps(sdc, LOGPIXELSX as i32);
        let dpi_y = GetDeviceCaps(sdc, LOGPIXELSY as i32);
        DPI_SCALE_X.store(dpi_x, Ordering::Relaxed);
        DPI_SCALE_Y.store(dpi_y, Ordering::Relaxed);
        ReleaseDC(std::ptr::null_mut(), sdc);

        let scale = dpi_y as f64 / 96.0;
        let ww = (WW as f64 * scale) as i32;
        let wh = (WH as f64 * scale) as i32;
        let gap = (GAP as f64 * scale) as i32;

        let cls = to_utf16(WINDOW_CLASS);

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: inst,
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: GetStockObject(BLACK_BRUSH) as HBRUSH,
            lpszMenuName: std::ptr::null(),
            lpszClassName: cls.as_ptr(),
        };

        if RegisterClassW(&wc) == 0 {
            log::error!("注册窗口类失败");
            return;
        }

        let mut wa = RECT::default();
        SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut wa as *mut _ as *mut std::ffi::c_void, 0);

        let sx = wa.right - ww - gap;
        let sy = wa.bottom - wh - gap;

        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW,
            cls.as_ptr(), std::ptr::null(), WS_POPUP,
            sx, sy, ww, wh,
            std::ptr::null_mut(), std::ptr::null_mut(), inst, std::ptr::null_mut(),
        );

        if hwnd.is_null() {
            log::error!("创建水印窗口失败");
            return;
        }

        WATERMARK_HWND.set(hwnd as isize).ok();
        SCREEN_X.store(sx, std::sync::atomic::Ordering::Relaxed);
        SCREEN_Y.store(sy, std::sync::atomic::Ordering::Relaxed);

        ShowWindow(hwnd, SW_SHOW);

        match CONFIG.get().unwrap().watermark.level {
            Level::TopMost => {
                SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
                render_frame(hwnd, sx, sy);
            }
            Level::Desktop => {
                set_desktop_level(hwnd);
                render_frame(hwnd, sx, sy);
                SetTimer(hwnd, TIMER_DESKTOP_LOWER, 50, None);
            }
        }

        if CONFIG.get().is_some_and(|c| c.watermark.colorful) {
            SetTimer(hwnd, TIMER_COLORFUL, 25, None);
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) != 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        log::info!("水印窗口已关闭");
    }
}

unsafe fn set_desktop_level(hwnd: *mut std::ffi::c_void) { unsafe {
    let pc = to_utf16("Progman");
    let progman = FindWindowW(pc.as_ptr(), std::ptr::null());
    if progman.is_null() {
        log::warn!("未找到 Progman 窗口, 回退到 TopMost 级别");
        SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
        return;
    }

    SendMessageW(progman, WM_PROGMAN_INIT, 0xD, 0x01);

    let wwc = to_utf16("WorkerW");
    let sdc = to_utf16("SHELLDLL_DefView");
    let mut workerw: *mut std::ffi::c_void = std::ptr::null_mut();
    loop {
        workerw = FindWindowExW(std::ptr::null_mut(), workerw, wwc.as_ptr(), std::ptr::null());
        if workerw.is_null() {
            break;
        }
        let defview = FindWindowExW(workerw, std::ptr::null_mut(), sdc.as_ptr(), std::ptr::null());
        if defview.is_null() {
            break;
        }
    }

    if !workerw.is_null() {
        SetParent(hwnd, workerw);
        SetWindowPos(hwnd, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    } else {
        SetParent(hwnd, progman);
        SetWindowPos(hwnd, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }
} }

fn render_frame(hwnd: *mut std::ffi::c_void, screen_x: i32, screen_y: i32) {
    unsafe {
        let Some(cfg) = CONFIG.get() else { return };
        let c = &cfg.watermark;

        let sdc = GetDC(std::ptr::null_mut());
        if sdc.is_null() { return }

        let mdc = CreateCompatibleDC(sdc);
        if mdc.is_null() {
            ReleaseDC(std::ptr::null_mut(), sdc);
            return;
        }

        let dpi_y = GetDeviceCaps(sdc, LOGPIXELSY as i32);
        let scale = dpi_y as f64 / 96.0;
        let ww = (WW as f64 * scale) as i32;
        let wh = (WH as f64 * scale) as i32;

        let mut bmi = std::mem::zeroed::<BITMAPINFO>();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = ww;
        bmi.bmiHeader.biHeight = -wh;
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB;

        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let dib = CreateDIBSection(mdc, &bmi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
        if dib.is_null() || bits.is_null() {
            DeleteDC(mdc);
            ReleaseDC(std::ptr::null_mut(), sdc);
            return;
        }

        let old_bmp = SelectObject(mdc, dib as HGDIOBJ);

        let white_brush = GetStockObject(WHITE_BRUSH);
        let rc = RECT { left: 0, top: 0, right: ww, bottom: wh };
        FillRect(mdc, &rc, white_brush as HBRUSH);

        let weight = if c.bold { FW_BOLD as i32 } else { FW_NORMAL as i32 };

        let fsz1 = -(dpi_y * c.font_size1 as i32 / 72);
        let f1 = CreateFontW(
            fsz1, 0, 0, 0, weight, 0, 0, 0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            (DEFAULT_PITCH | FF_DONTCARE) as u32,
            to_utf16("Microsoft YaHei").as_ptr(),
        );
        let old_f1 = SelectObject(mdc, f1 as HGDIOBJ);
        SetTextColor(mdc, 0);
        SetBkMode(mdc, TRANSPARENT as i32);

        let l1 = to_utf16(&c.line1);
        let left1 = (12.0 * scale) as i32;
        let top1 = (8.0 * scale) as i32;
        let mut r1 = RECT { left: left1, top: top1, right: ww, bottom: wh };
        DrawTextW(mdc, l1.as_ptr(), -1, &mut r1, DT_LEFT | DT_TOP | DT_SINGLELINE);

        SelectObject(mdc, old_f1);
        DeleteObject(f1 as HGDIOBJ);

        let fsz2 = -(dpi_y * c.font_size2 as i32 / 72);
        let f2 = CreateFontW(
            fsz2, 0, 0, 0, weight, 0, 0, 0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            (DEFAULT_PITCH | FF_DONTCARE) as u32,
            to_utf16("Microsoft YaHei").as_ptr(),
        );
        let old_f2 = SelectObject(mdc, f2 as HGDIOBJ);

        let l2 = to_utf16(&c.line2);
        let top2 = (42.0 * scale) as i32;
        let bot2 = wh - (4.0 * scale) as i32;
        let mut r2 = RECT { left: left1, top: top2, right: ww, bottom: bot2 };
        DrawTextW(mdc, l2.as_ptr(), -1, &mut r2, DT_LEFT | DT_TOP | DT_SINGLELINE);

        SelectObject(mdc, old_f2);
        DeleteObject(f2 as HGDIOBJ);

        let stc = config::parse_color(&c.color).unwrap_or(0x00A6A7A8);
        let sr = stc & 0xFF;
        let sg = (stc >> 8) & 0xFF;
        let sb = (stc >> 16) & 0xFF;
        let hue_ofs = HUE_OFFSET.load(Ordering::Relaxed);
        let op = 220u32;

        let pixels: &mut [u32] = std::slice::from_raw_parts_mut(
            bits as *mut u32,
            (ww * wh) as usize,
        );

        for y in 0..wh {
            for x in 0..ww {
                let p = &mut pixels[(y * ww + x) as usize];
                let b = *p & 0xFF;
                let g = (*p >> 8) & 0xFF;
                let r = (*p >> 16) & 0xFF;
                let lum = (r + g + b) as u32 / 3;
                let a = ((255u32.saturating_sub(lum)) * op) / 255;
                if a == 0 {
                    *p = 0;
                    continue;
                }
                let (cr, cg, cb) = if c.colorful {
                    let hue = ((x as u32 * 4 + hue_ofs) % 360) as f32;
                    let col = config::hsl_to_rgb(hue, 1.0, 0.5);
                    (col & 0xFF, (col >> 8) & 0xFF, (col >> 16) & 0xFF)
                } else {
                    (sr, sg, sb)
                };
                let pm = |ch: u32| -> u32 { (ch * a + 127) / 255 };
                *p = (a << 24) | (pm(cr) << 16) | (pm(cg) << 8) | pm(cb);
            }
        }

        let dp = POINT { x: screen_x, y: screen_y };
        let sp = POINT { x: 0, y: 0 };
        let sz = SIZE { cx: ww, cy: wh };
        let bl = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };

        UpdateLayeredWindow(
            hwnd, sdc, &dp, &sz, mdc, &sp, 0, &bl, ULW_ALPHA,
        );

        SelectObject(mdc, old_bmp);
        DeleteObject(dib as HGDIOBJ);
        DeleteDC(mdc);
        ReleaseDC(std::ptr::null_mut(), sdc);
    }
}

unsafe extern "system" fn window_proc(
    hwnd: *mut std::ffi::c_void,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    match msg {
        WM_PAINT => unsafe {
            let mut ps = std::mem::zeroed::<PAINTSTRUCT>();
            BeginPaint(hwnd, &mut ps);
            render_frame(
                hwnd,
                SCREEN_X.load(std::sync::atomic::Ordering::Relaxed),
                SCREEN_Y.load(std::sync::atomic::Ordering::Relaxed),
            );
            EndPaint(hwnd, &ps);
            0
        },
        WM_TIMER => {
            if wparam == TIMER_COLORFUL {
                HUE_OFFSET.fetch_add(4, Ordering::Relaxed);
                render_frame(
                    hwnd,
                    SCREEN_X.load(std::sync::atomic::Ordering::Relaxed),
                    SCREEN_Y.load(std::sync::atomic::Ordering::Relaxed),
                );
            } else if wparam == TIMER_DESKTOP_LOWER {
                unsafe {
                    SetWindowPos(hwnd, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                    KillTimer(hwnd, TIMER_DESKTOP_LOWER);
                }
            }
            0
        },
        WM_DESTROY => unsafe {
            if CONFIG.get().is_some_and(|c| c.watermark.colorful) {
                KillTimer(hwnd, TIMER_COLORFUL);
            }
            KillTimer(hwnd, TIMER_DESKTOP_LOWER);
            PostQuitMessage(0);
            0
        },
        WM_ERASEBKGND => 1,
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

#[allow(dead_code)]
pub fn close() {
    if let Some(&hwnd) = WATERMARK_HWND.get() {
        unsafe {
            PostMessageW(hwnd as *mut std::ffi::c_void, WM_CLOSE, 0, 0);
        }
    }
}

fn to_utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
