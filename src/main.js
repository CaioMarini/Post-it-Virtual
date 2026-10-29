const { invoke, convertFileSrc } = window.__TAURI__.core;
const COLORS = ["yellow", "pink", "green", "blue", "purple"];
const $ = (id) => document.getElementById(id);
const text = $("text");
let note, imagesDir;

function render() {
  document.body.dataset.color = note.color;
  $("layer").classList.toggle("off", !note.on_top);
  $("layer").title = note.on_top ? "Mandar para trás" : "Trazer para frente";
}

// O src das imagens é recalculado a cada abertura; no HTML salvo vale só o data-file.
function fixImages() {
  for (const img of text.querySelectorAll("img[data-file]")) {
    img.src = convertFileSrc(imagesDir + img.dataset.file);
  }
}

let timer;
const save = () => {
  clearTimeout(timer);
  invoke("set_html", { html: text.innerHTML });
};
text.addEventListener("input", () => {
  if (!text.textContent.trim() && !text.querySelector("img")) text.innerHTML = "";
  clearTimeout(timer);
  timer = setTimeout(save, 300);
});
text.addEventListener("blur", save);

// Imagem colada vira arquivo; texto entra sem formatação.
text.addEventListener("paste", async (e) => {
  e.preventDefault();
  const file = [...e.clipboardData.files].find((f) => f.type.startsWith("image/"));
  if (!file) {
    document.execCommand("insertText", false, e.clipboardData.getData("text/plain"));
    return;
  }
  const bytes = new Uint8Array(await file.arrayBuffer());
  const name = await invoke("save_image", bytes, { headers: { ext: file.type.split("/")[1] } });
  document.execCommand("insertHTML", false, `<img data-file="${name}" src="${convertFileSrc(imagesDir + name)}">`);
});

$("new").onclick = () => invoke("new_note");

$("color").onclick = () => {
  note.color = COLORS[(COLORS.indexOf(note.color) + 1) % COLORS.length];
  render();
  invoke("set_color", { color: note.color });
};

$("layer").onclick = () => {
  note.on_top = !note.on_top;
  render();
  invoke("set_on_top", { onTop: note.on_top });
};

$("del").onclick = () => invoke("delete_note");

Promise.all([invoke("get_note"), invoke("images_dir")]).then(([n, dir]) => {
  note = n;
  imagesDir = dir;
  text.innerHTML = n.html;
  fixImages();
  render();
  text.focus();
});
