//! Modo "atrás": mantém a nota no fundo da pilha de janelas, mesmo quando ela é clicada.
//! O Windows tenta trazer a janela para frente ao ativá-la; interceptamos WM_WINDOWPOSCHANGING
//! e trocamos o destino para HWND_BOTTOM.

use std::sync::Mutex;
use tauri::WebviewWindow;
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{
        Shell::{DefSubclassProc, SetWindowSubclass},
        WindowsAndMessaging::{
            SetWindowPos, HWND_BOTTOM, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WINDOWPOS,
            WM_WINDOWPOSCHANGING,
        },
    },
};

static BOTTOM: Mutex<Vec<isize>> = Mutex::new(Vec::new());

fn hwnd(w: &WebviewWindow) -> HWND {
    HWND(w.hwnd().expect("janela sem HWND").0 as _)
}

/// Deve ser chamada na thread principal (dona da janela).
pub fn install(w: &WebviewWindow) {
    unsafe {
        let _ = SetWindowSubclass(hwnd(w), Some(subclass_proc), 1, 0);
    }
}

pub fn set_bottom(w: &WebviewWindow, on: bool) {
    let h = hwnd(w);
    {
        let mut list = BOTTOM.lock().unwrap();
        list.retain(|&x| x != h.0 as isize);
        if on {
            list.push(h.0 as isize);
        }
    } // solta o lock antes do SetWindowPos, que dispara subclass_proc de forma síncrona
    if on {
        unsafe {
            let _ = SetWindowPos(h, Some(HWND_BOTTOM), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
    }
}

unsafe extern "system" fn subclass_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _id: usize, _data: usize) -> LRESULT {
    if msg == WM_WINDOWPOSCHANGING && BOTTOM.lock().unwrap().contains(&(h.0 as isize)) {
        let pos = &mut *(lp.0 as *mut WINDOWPOS);
        pos.hwndInsertAfter = HWND_BOTTOM;
        pos.flags &= !SWP_NOZORDER;
    }
    DefSubclassProc(h, msg, wp, lp)
}
