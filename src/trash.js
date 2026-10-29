const { invoke } = window.__TAURI__.core;
const $ = (id) => document.getElementById(id);
let notes = [];

const when = (ms) => new Date(ms).toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" });

// DOMParser não carrega as imagens; só queremos o texto e saber se há imagem.
function preview(html) {
  const body = new DOMParser().parseFromString(html, "text/html").body;
  // Fora da tela, innerText não quebra linhas; quebra manualmente em <br> e <div>.
  body.querySelectorAll("br").forEach((br) => br.replaceWith("\n"));
  body.querySelectorAll("div").forEach((div) => div.prepend("\n"));
  return { text: body.innerText.trim(), hasImage: !!body.querySelector("img") };
}

function button(label, title, onclick) {
  const b = document.createElement("button");
  b.textContent = label;
  b.title = title;
  b.onclick = onclick;
  return b;
}

function card(n) {
  const { text, hasImage } = preview(n.html);
  const el = document.createElement("article");
  el.dataset.color = n.color;

  const p = document.createElement("p");
  p.textContent = (hasImage ? "🖼 " : "") + (text || "(vazia)");

  const meta = document.createElement("small");
  meta.textContent = "Apagada em " + when(n.deleted_at);

  const restore = button("Restaurar", "Reabrir a nota", async () => {
    await invoke("restore_note", { id: n.id });
    refresh();
  });
  // Apagar de vez pede um segundo clique.
  const purge = button("🗑", "Apagar de vez", async () => {
    if (!purge.classList.contains("confirm")) {
      purge.classList.add("confirm");
      purge.textContent = "Apagar de vez?";
      setTimeout(() => {
        purge.classList.remove("confirm");
        purge.textContent = "🗑";
      }, 2500);
      return;
    }
    await invoke("purge_note", { id: n.id });
    refresh();
  });

  const actions = document.createElement("div");
  actions.append(meta, restore, purge);
  el.append(p, actions);
  return el;
}

function render() {
  const q = $("search").value.trim().toLowerCase();
  const shown = notes.filter((n) => !q || preview(n.html).text.toLowerCase().includes(q));
  $("list").replaceChildren(...shown.map(card));
  if (!shown.length) {
    const empty = document.createElement("p");
    empty.className = "empty";
    empty.textContent = q ? "Nada encontrado." : "A lixeira está vazia.";
    $("list").append(empty);
  }
}

async function refresh() {
  notes = await invoke("list_trash");
  render();
}

$("search").addEventListener("input", render);
// Notas podem ir para a lixeira com esta janela aberta; atualiza ao voltar para ela.
window.addEventListener("focus", refresh);
refresh();
