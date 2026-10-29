# Post-it Virtual

Notas adesivas para anotações rápidas no Windows, feitas com [Tauri 2](https://tauri.app).

- Cada nota é uma janela própria, que abre **por cima de todos os apps**.
- O botão 📌 manda a nota **para o fundo**, e ela fica atrás das outras janelas mesmo quando você clica nela para digitar.
- Cola imagens com `Ctrl+V`, como prints do `Win+Shift+S`.
- O **×** manda a nota para a **lixeira**, que fica no ícone da bandeja. Lá dá para restaurar a nota ou apagar de vez, e o que estiver lá há mais de 30 dias é apagado sozinho.
- `Ctrl+Alt+N` cria uma nota nova de qualquer lugar.
- Cor, posição, tamanho e conteúdo ficam salvos em `%APPDATA%\com.caio.notasadesivas\`.

## Rodando

Requisitos: [Rust](https://rustup.rs), Node.js e Visual Studio Build Tools com a carga de trabalho "C++".

```sh
npm install
npm run dev     # modo desenvolvimento
npm run build   # instalador em src-tauri/target/release/bundle/nsis/
```

## Estrutura

- `src/`: interface em HTML/CSS/JS puro (`index.html` é a nota, `trash.html` é a lixeira)
- `src-tauri/src/main.rs`: janelas, persistência, bandeja e atalho
- `src-tauri/src/layer.rs`: o modo "atrás", via subclasse Win32 (`WM_WINDOWPOSCHANGING`)
