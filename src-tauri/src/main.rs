#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod layer;

use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{
    ipc::{InvokeBody, Request},
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager, PhysicalPosition, PhysicalSize, RunEvent, State, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const TRASH_DAYS: u64 = 30;

#[derive(Clone, Serialize, Deserialize)]
struct Note {
    id: u64, // também é o momento de criação, em ms
    #[serde(default)]
    html: String,
    // Formato antigo (só texto); migrado para `html` ao carregar.
    #[serde(default, skip_serializing)]
    text: String,
    color: String,
    on_top: bool,
    pos: Option<(i32, i32)>,
    size: Option<(u32, u32)>,
    #[serde(default)]
    updated_at: u64,
    #[serde(default)]
    deleted_at: Option<u64>,
}

struct Notes {
    path: PathBuf,
    images: PathBuf,
    list: Mutex<Vec<Note>>,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('\n', "<br>")
}

fn remove_images(dir: &Path, id: u64) {
    let prefix = format!("{id}-");
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        if entry.file_name().to_string_lossy().starts_with(&prefix) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

impl Notes {
    fn load(dir: PathBuf) -> Self {
        let path = dir.join("notes.json");
        let images = dir.join("images");
        let mut list: Vec<Note> = match fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_else(|_| {
                // Arquivo corrompido: guarda uma cópia antes de começar do zero.
                let _ = fs::copy(&path, path.with_extension("json.bak"));
                Vec::new()
            }),
            Err(_) => Vec::new(),
        };
        for n in &mut list {
            if n.html.is_empty() && !n.text.is_empty() {
                n.html = escape_html(&n.text);
            }
        }
        let cutoff = now_ms().saturating_sub(TRASH_DAYS * 24 * 60 * 60 * 1000);
        list.retain(|n| match n.deleted_at {
            Some(t) if t < cutoff => {
                remove_images(&images, n.id);
                false
            }
            _ => true,
        });
        let notes = Self { path, images, list: Mutex::new(Vec::new()) };
        notes.save(&list);
        *notes.list.lock().unwrap() = list;
        notes
    }

    fn save(&self, list: &[Note]) {
        if let Some(dir) = self.path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        // Grava num temporário e renomeia, para não deixar o arquivo pela metade.
        let tmp = self.path.with_extension("json.tmp");
        if fs::write(&tmp, serde_json::to_string_pretty(list).unwrap()).is_ok() {
            let _ = fs::rename(&tmp, &self.path);
        }
    }

    fn edit(&self, id: u64, f: impl FnOnce(&mut Note)) {
        let mut list = self.list.lock().unwrap();
        if let Some(n) = list.iter_mut().find(|n| n.id == id) {
            f(n);
        }
        self.save(&list);
    }
}

fn note_id(w: &WebviewWindow) -> u64 {
    w.label().trim_start_matches("note-").parse().unwrap_or(0)
}

// No Windows, criar um WebView2 a partir de um handler na thread principal trava o app;
// por isso cada janela abre numa thread própria.
fn spawn_note(app: &AppHandle, note: Note) {
    let app = app.clone();
    std::thread::spawn(move || {
        if let Err(e) = open_note(&app, note) {
            eprintln!("falha ao abrir nota: {e}");
        }
    });
}

fn open_note(app: &AppHandle, note: Note) -> tauri::Result<()> {
    let mut builder = WebviewWindowBuilder::new(app, format!("note-{}", note.id), WebviewUrl::App("index.html".into()))
        .title("Nota")
        .decorations(false)
        .skip_taskbar(true)
        .always_on_top(note.on_top)
        .inner_size(260.0, 260.0)
        .min_inner_size(160.0, 120.0)
        .visible(false);
    if note.pos.is_none() {
        builder = builder.center();
    }
    let win = builder.build()?;
    if let Some((x, y)) = note.pos {
        win.set_position(PhysicalPosition::new(x, y))?;
    }
    if let Some((w, h)) = note.size {
        win.set_size(PhysicalSize::new(w, h))?;
    }

    let handle = app.clone();
    let id = note.id;
    win.on_window_event(move |event| {
        let notes = handle.state::<Notes>();
        match event {
            WindowEvent::Moved(p) => notes.edit(id, |n| n.pos = Some((p.x, p.y))),
            WindowEvent::Resized(s) if s.width > 0 => notes.edit(id, |n| n.size = Some((s.width, s.height))),
            _ => {}
        }
    });

    // A subclasse Win32 só pode ser instalada na thread dona da janela (a principal).
    let on_top = note.on_top;
    app.run_on_main_thread(move || {
        layer::install(&win);
        let _ = win.show();
        if !on_top {
            layer::set_bottom(&win, true);
        }
    })
}

fn create_note(app: &AppHandle) {
    let now = now_ms();
    let note = Note {
        id: now,
        html: String::new(),
        text: String::new(),
        color: "yellow".into(),
        on_top: true,
        pos: None,
        size: None,
        updated_at: now,
        deleted_at: None,
    };
    let notes = app.state::<Notes>();
    {
        let mut list = notes.list.lock().unwrap();
        list.push(note.clone());
        notes.save(&list);
    }
    spawn_note(app, note);
}

fn open_trash(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("trash") {
        let _ = w.unminimize();
        let _ = w.set_focus();
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let _ = WebviewWindowBuilder::new(&app, "trash", WebviewUrl::App("trash.html".into()))
            .title("Lixeira — Notas Adesivas")
            .inner_size(380.0, 520.0)
            .build();
    });
}

#[tauri::command]
fn get_note(window: WebviewWindow, notes: State<Notes>) -> Option<Note> {
    let id = note_id(&window);
    notes.list.lock().unwrap().iter().find(|n| n.id == id).cloned()
}

#[tauri::command]
fn set_html(window: WebviewWindow, notes: State<Notes>, html: String) {
    notes.edit(note_id(&window), |n| {
        n.html = html;
        n.updated_at = now_ms();
    });
}

#[tauri::command]
fn set_color(window: WebviewWindow, notes: State<Notes>, color: String) {
    notes.edit(note_id(&window), |n| n.color = color);
}

#[tauri::command]
fn set_on_top(window: WebviewWindow, notes: State<Notes>, on_top: bool) -> Result<(), String> {
    if on_top {
        layer::set_bottom(&window, false);
        window.set_always_on_top(true).map_err(|e| e.to_string())?;
    } else {
        window.set_always_on_top(false).map_err(|e| e.to_string())?;
        layer::set_bottom(&window, true);
    }
    notes.edit(note_id(&window), |n| n.on_top = on_top);
    Ok(())
}

#[tauri::command]
fn new_note(app: AppHandle) {
    create_note(&app);
}

/// Manda a nota para a lixeira e fecha a janela.
#[tauri::command]
fn delete_note(window: WebviewWindow, notes: State<Notes>) -> Result<(), String> {
    notes.edit(note_id(&window), |n| n.deleted_at = Some(now_ms()));
    layer::set_bottom(&window, false);
    window.destroy().map_err(|e| e.to_string())
}

#[tauri::command]
fn images_dir(notes: State<Notes>) -> String {
    format!("{}\\", notes.images.display())
}

/// Recebe os bytes da imagem colada e devolve o nome do arquivo salvo.
#[tauri::command]
fn save_image(window: WebviewWindow, notes: State<Notes>, request: Request) -> Result<String, String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("esperava bytes da imagem".into());
    };
    let ext = request.headers().get("ext").and_then(|v| v.to_str().ok()).unwrap_or("");
    if !["png", "jpeg", "gif", "webp", "bmp"].contains(&ext) {
        return Err(format!("formato de imagem não suportado: {ext}"));
    }
    let name = format!("{}-{}.{}", note_id(&window), now_ms(), ext);
    fs::create_dir_all(&notes.images).map_err(|e| e.to_string())?;
    fs::write(notes.images.join(&name), bytes).map_err(|e| e.to_string())?;
    Ok(name)
}

#[tauri::command]
fn list_trash(notes: State<Notes>) -> Vec<Note> {
    let mut trash: Vec<Note> = notes.list.lock().unwrap().iter().filter(|n| n.deleted_at.is_some()).cloned().collect();
    trash.sort_by_key(|n| std::cmp::Reverse(n.deleted_at));
    trash
}

#[tauri::command]
fn restore_note(app: AppHandle, notes: State<Notes>, id: u64) {
    notes.edit(id, |n| n.deleted_at = None);
    let note = notes.list.lock().unwrap().iter().find(|n| n.id == id).cloned();
    if let Some(note) = note {
        spawn_note(&app, note);
    }
}

#[tauri::command]
fn purge_note(notes: State<Notes>, id: u64) {
    let mut list = notes.list.lock().unwrap();
    list.retain(|n| n.id != id);
    notes.save(&list);
    remove_images(&notes.images, id);
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            get_note,
            set_html,
            set_color,
            set_on_top,
            new_note,
            delete_note,
            images_dir,
            save_image,
            list_trash,
            restore_note,
            purge_note
        ])
        .setup(|app| {
            app.manage(Notes::load(app.path().app_data_dir()?));

            let menu = Menu::with_items(
                app,
                &[
                    &MenuItem::with_id(app, "new", "Nova nota  (Ctrl+Alt+N)", true, None::<&str>)?,
                    &MenuItem::with_id(app, "trash", "Lixeira", true, None::<&str>)?,
                    &MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?,
                ],
            )?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Notas Adesivas")
                .menu(&menu)
                .on_menu_event(|app, e| match e.id.as_ref() {
                    "new" => create_note(app),
                    "trash" => open_trash(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            // Se outro programa já usa o atalho, o app segue funcionando pela bandeja.
            if let Err(e) = app.global_shortcut().on_shortcut("ctrl+alt+n", |app, _, ev| {
                if ev.state == ShortcutState::Pressed {
                    create_note(app);
                }
            }) {
                eprintln!("atalho Ctrl+Alt+N indisponível: {e}");
            }

            let active: Vec<Note> =
                app.state::<Notes>().list.lock().unwrap().iter().filter(|n| n.deleted_at.is_none()).cloned().collect();
            if active.is_empty() {
                create_note(app.handle());
            }
            for note in active {
                spawn_note(app.handle(), note);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("erro ao iniciar")
        .run(|_, event| {
            // Fechar a última janela não encerra o app; ele continua na bandeja.
            if let RunEvent::ExitRequested { code: None, api, .. } = event {
                api.prevent_exit();
            }
        });
}
