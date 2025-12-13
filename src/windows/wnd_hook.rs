use super::gui_impl::input;
use crate::il2cpp::hook::umamusume::StandaloneWindowResize::get_IsVirt;
use crate::il2cpp::types::Il2CppImage;
use crate::windows::game_impl;
use crate::windows::game_impl::is_steam_release;
use crate::{core::{game::Region, Gui, Hachimi}, il2cpp::{hook::{umamusume::SceneManager, UnityEngine_CoreModule}, symbols::Thread}, windows::utils};
use egui::mutex::Mutex;
use once_cell::sync::Lazy;
use std::ptr::null_mut;
use std::{os::raw::c_uint, sync::atomic::{self, AtomicIsize}};
use windows::Win32::Foundation::TRUE;
use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, GetWindowRect, SetWindowPos, HWND_NOTOPMOST, SWP_DEFERERASE, WMSZ_LEFT, WMSZ_RIGHT, WM_SIZING};
use windows::{core::w, Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    System::Threading::GetCurrentThreadId,
    UI::WindowsAndMessaging::{
        CallNextHookEx, DefWindowProcW, FindWindowW, GetWindowLongPtrW, SetWindowsHookExW, UnhookWindowsHookEx,
        GWLP_WNDPROC, HCBT_MINMAX, HHOOK, SW_RESTORE, WH_CBT, WMSZ_BOTTOMLEFT, WMSZ_TOP, WMSZ_TOPLEFT, WMSZ_TOPRIGHT, WM_CLOSE,
        WM_KEYDOWN, WM_SIZE, WM_SYSKEYDOWN, WNDPROC
    }
}};

struct WndProcCall {
    hwnd: HWND,
    umsg: c_uint,
    wparam: WPARAM,
    lparam: LPARAM
}

static WM_SIZE_BUFFER: Lazy<Mutex<Vec<WndProcCall>>> = Lazy::new(|| Mutex::default());
pub fn drain_wm_size_buffer() {
    let Some(orig_fn) = (unsafe { std::mem::transmute::<isize, WNDPROC>(WNDPROC_ORIG) }) else {
        return;
    };
    for call in WM_SIZE_BUFFER.lock().drain(..) {
        unsafe { orig_fn(call.hwnd, call.umsg, call.wparam, call.lparam); }
    }
}

static TARGET_HWND: AtomicIsize = AtomicIsize::new(0);
pub fn get_target_hwnd() -> HWND {
    HWND(TARGET_HWND.load(atomic::Ordering::Relaxed))
}

// Safety: only modified once on init
static mut WNDPROC_ORIG: isize = 0;
static mut WNDPROC_RECALL: usize = 0;
extern "system" fn wnd_proc(hwnd: HWND, umsg: c_uint, wparam: WPARAM, lparam: LPARAM) -> LRESULT {

    let Some(orig_fn) = (unsafe { std::mem::transmute::<isize, WNDPROC>(WNDPROC_ORIG) }) else {
        return unsafe { DefWindowProcW(hwnd, umsg, wparam, lparam) };
    };

    match umsg {
        // Check for Home key presses
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            if wparam.0 as u16 == Hachimi::instance().config.load().windows.menu_open_key {
                let Some(mut gui) = Gui::instance().map(|m| m.lock().unwrap()) else {
                    return unsafe { orig_fn(hwnd, umsg, wparam, lparam) };
                };

                gui.toggle_menu();
                return LRESULT(0);
            }
        },
        WM_CLOSE => {
            if let Some(hook) = Hachimi::instance().interceptor.unhook(wnd_proc as _) {
                unsafe { WNDPROC_RECALL = hook.orig_addr; }
                Thread::main_thread().schedule(|| {
                    unsafe {
                        let orig_fn = std::mem::transmute::<usize, WNDPROC>(WNDPROC_RECALL).unwrap();
                        orig_fn(get_target_hwnd(), WM_CLOSE, WPARAM(0), LPARAM(0));
                    }
                });
            }
            return LRESULT(0);
        },
        WM_SIZE => {
            // if !SceneManager::is_splash_shown() {
            //     WM_SIZE_BUFFER.lock().push(WndProcCall {
            //         hwnd, umsg, wparam, lparam
            //     });
            //     return LRESULT(0);
            // }
            // else {
                return unsafe { orig_fn(hwnd, umsg, wparam, lparam) };
            // }
        },
        WM_SIZING=>{
            return unlock_size(hwnd, umsg, wparam, lparam);
        },
        _ => ()
    }

    // Only capture input if gui needs it
    if !Gui::is_consuming_input_atomic() {
        return unsafe { orig_fn(hwnd, umsg, wparam, lparam) };
    }

    // Check if the input processor handles this message
    if !input::is_handled_msg(umsg) {
        return unsafe { orig_fn(hwnd, umsg, wparam, lparam) };
    }

    // A deadlock would *sometimes* consistently occur if this was done on the current thread
    // (when moving the window, etc.)
    // I assume that SwapChain::Present and WndProc are running on the same thread
    std::thread::spawn(move || {
        let Some(mut gui) = Gui::instance().map(|m| m.lock().unwrap()) else {
            return;
        };

        let zoom_factor = gui.context.zoom_factor();
        input::process(&mut gui.input, zoom_factor, umsg, wparam.0, lparam.0);
    });

    LRESULT(0)
}



static mut HCBTHOOK: HHOOK = HHOOK(0);
extern "system" fn cbt_proc(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if ncode == HCBT_MINMAX as i32 &&
        lparam.0 as i32 != SW_RESTORE.0 &&
        Hachimi::instance().config.load().windows.block_minimize_in_full_screen &&
        UnityEngine_CoreModule::Screen::get_fullScreen()
    {
        return LRESULT(1);
    }

    unsafe { CallNextHookEx(HCBTHOOK, ncode, wparam, lparam) }
}

pub fn init() {
    unsafe {
        let hachimi = Hachimi::instance();
        let game = &hachimi.game;

        let window_name = if game.region == Region::Japan && game.is_steam_release {
            // lmao
            w!("UmamusumePrettyDerby_Jpn")
        }
        else {
            // global technically has "Umamusume" as its title but this api
            // is case insensitive so it works. why am i surprised
            w!("umamusume")
        };
        let hwnd = FindWindowW(w!("UnityWndClass"), window_name);
        if hwnd.0 == 0 {
            error!("Failed to find game window");
            return;
        }
        TARGET_HWND.store(hwnd.0, atomic::Ordering::Relaxed);

        info!("Hooking WndProc");
        let wnd_proc_addr = GetWindowLongPtrW(hwnd, GWLP_WNDPROC);
        match hachimi.interceptor.hook(wnd_proc_addr as _, wnd_proc as _) {
            Ok(trampoline_addr) => WNDPROC_ORIG = trampoline_addr as _,
            Err(e) => error!("Failed to hook WndProc: {}", e)
        }

        info!("Adding CBT hook");
        if let Ok(hhook) = SetWindowsHookExW(WH_CBT, Some(cbt_proc), None, GetCurrentThreadId()) {
            HCBTHOOK = hhook;
        }

        // Apply always on top
        if hachimi.window_always_on_top.load(atomic::Ordering::Relaxed) {
            _ = utils::set_window_topmost(hwnd, true);
        }
    }
}

pub fn uninit() {
    unsafe {
        if HCBTHOOK.0 != 0 {
            info!("Removing CBT hook");
            if let Err(e) = UnhookWindowsHookEx(HCBTHOOK) {
                error!("Failed to remove CBT hook: {}", e);
            }
            HCBTHOOK = HHOOK(0);
        }
    }
}

static mut last_height:i32=0;
static mut last_width:i32=0;
static g_aspect_ratio:f32=16f32/9f32;
static g_force_landscape:bool = false;

fn unlock_size(hwnd: HWND, umsg: c_uint, wparam: WPARAM, lparam: LPARAM) ->LRESULT {
    let rect_ptr = lparam.0 as *mut RECT;

    if rect_ptr.is_null() {
        return LRESULT(0);
    }

    let rect: &mut RECT = &mut unsafe { *rect_ptr };

    let is_vert = is_virt();

    if !is_vert || wparam.0 as u32 == WMSZ_LEFT || wparam.0 as u32 == WMSZ_RIGHT {
        let ret = update_window_ratio(hwnd, rect, wparam, false);
        rect.left = ret.left;
        rect.right = ret.right;
        rect.top = ret.top;
        rect.bottom = ret.bottom;

        return LRESULT(1);
    }

    let ratio: f32 = if is_vert {
        1.0 / g_aspect_ratio
    } else {
        g_aspect_ratio
    };

    let mut height = (rect.bottom - rect.top) as f32;
    let mut width = (rect.right - rect.left) as f32;

    let new_ratio = width / height;

    let last_h = unsafe { last_height } as f32;
    let last_w = unsafe { last_width } as f32;

    if (new_ratio > ratio && height >= last_h) || (width < last_w) {
        height = width / ratio;
    }
    else if (new_ratio < ratio && width >= last_w) || (height < last_h) {
        width = height * ratio;
    }

    match wparam.0 as u32 {
        WMSZ_TOP | WMSZ_TOPLEFT | WMSZ_TOPRIGHT => {
            rect.top = rect.bottom - height.round() as i32;
        }
        _ => {
            rect.bottom = rect.top + height.round() as i32;
        }
    }

    match wparam.0 as u32 {
        WMSZ_LEFT | WMSZ_TOPLEFT | WMSZ_BOTTOMLEFT => {
            rect.left = rect.right - width.round() as i32;
        }
        _ => {
            rect.right = rect.left + width.round() as i32;
        }
    }

    unsafe {
        last_height = height as i32;
        last_width = width as i32;
    }

    LRESULT(1)
}

fn update_window_ratio(hwnd: HWND, modified_r:&mut RECT, wparam: WPARAM, resize_now:bool) ->RECT{
    let mut window_r: RECT = Default::default();
    let mut client_r: RECT = Default::default();

    unsafe {
        if let Err(err)=GetWindowRect(hwnd, &mut window_r){
            error!("Error getting window rect {:?}: {}", hwnd, err);
        }
        if let Err(err)=GetClientRect(hwnd, &mut client_r){
            error!("Error getting client rect {:?}: {}", hwnd, err);
        }
    }
    let mut add_w = (modified_r.right - modified_r.left) as f32 - (window_r.right - window_r.left) as f32;
    let mut add_h = (modified_r.bottom - modified_r.top) as f32 - (window_r.bottom - window_r.top) as f32;

    if add_h != 0.0 {
        add_w = add_h * g_aspect_ratio;
    } else {
        add_h = add_w / g_aspect_ratio;
    }

    let X = window_r.left;
    let Y = window_r.top;
    let mut cx = client_r.right as f32;
    let mut cy;

    let is_vert = is_virt();
    if is_vert {
        cy = cx * g_aspect_ratio;
    } else {
        cy = cx / g_aspect_ratio;
    }

    cx += add_w;
    cy += add_h;

    let new_width = cx + (window_r.right - window_r.left - client_r.right) as f32;
    let new_height = cy + (window_r.bottom - window_r.top - client_r.bottom) as f32;

    let mut new_window_r: RECT = Default::default();
    new_window_r.left = X;
    new_window_r.top = Y;
    new_window_r.right = X + new_width.round() as i32;
    new_window_r.bottom = Y + new_height.round() as i32;

    match wparam.0 as u32 {
        WMSZ_TOP | WMSZ_TOPLEFT | WMSZ_TOPRIGHT => {
            new_window_r.top = (new_window_r.top as f32 - add_h).round() as i32;
        }
        _ => {}
    }

    match wparam.0 as u32 {
        WMSZ_LEFT | WMSZ_TOPLEFT | WMSZ_BOTTOMLEFT => {
            new_window_r.left = (new_window_r.left as f32 - add_w).round() as i32;
        }
        _ => {}
    }

    if resize_now {
        let swp_cx = new_window_r.right - new_window_r.left;
        let swp_cy = new_window_r.bottom - new_window_r.top;

        unsafe {
            if let Err(err)=SetWindowPos(
                hwnd,
                HWND_NOTOPMOST,
                new_window_r.left,
                new_window_r.top,
                swp_cx,
                swp_cy,
                SWP_DEFERERASE,
            ){
                error!("Error setting window pos {:?}: {}", hwnd, err);
            }
        }
    }

    new_window_r
}

fn is_virt() -> bool {
    if g_force_landscape{
        return false;
    }
    get_IsVirt()
}
