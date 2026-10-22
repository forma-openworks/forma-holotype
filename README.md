<p align="center">
  <img src="assets/logo/holotype.webp" width="128" alt="Forma: Holotype logo">
</p>

<h1 align="center">Forma: Holotype</h1>

<p align="center">
  <b>Notes, study notes and documentation for FORMA.</b><br>
  Markdown · wiki links · link graph · LaTeX formulas · diagrams
</p>

<p align="center">
  <img alt="Status: early development" src="https://img.shields.io/badge/status-early%20development-8a6cf0">
  <img alt="Rust 2024" src="https://img.shields.io/badge/Rust-2024-f46623?logo=rust&logoColor=white">
  <img alt="Rendering: wgpu + Vulkan" src="https://img.shields.io/badge/render-wgpu%20%2B%20Vulkan-a41e22">
  <img alt="Languages: English, Russian" src="https://img.shields.io/badge/UI-English%20%7C%20%D0%A0%D1%83%D1%81%D1%81%D0%BA%D0%B8%D0%B9-4b8bbe">
</p>

<p align="center">
  <a href="#english">English</a> · <a href="#русский">Русский</a>
</p>

---

## English

### What it is

Forma: Holotype is a desktop app for notes, study notes and documentation in the spirit of Obsidian. It is part of FORMA and is built on [Forma Core](https://github.com/forma-openworks/forma-core): "different apps, one base". The window, areas, workspaces, themes, settings, keymap, Find Action, plugins and translations are shared with [Forma Mycelium](https://github.com/forma-openworks/forma-mycelium). Holotype brings only its own parts: notes, the editor, the reading view, the graph, formulas, diagrams and export.

Notes are plain `.md` files in a folder (a "vault"), so they open in any editor and can live in git.

A holotype is the first specimen a new species is described from.

### What already works

**Notes and links**
- **A vault** with a folder tree: create, rename (links to the note are updated everywhere), drag between folders, delete to the Recycle Bin.
- **`[[Wiki links]]`**, backlinks, `#tags`, an outline and note info.
- **A link graph** (`Ctrl+G`) with force-directed layout. A dragged node stays where you drop it and the layout is saved in the vault (`.holotype/graph.json`). Right-click unpins a node, "Reset" brings back the starting layout.
- **Quick open** (`Ctrl+O`) to find or create a note by name, full-text **search** (`Ctrl+Shift+F`), back and forward history for every note tab, a **daily note** (`Ctrl+D`).

**Writing**
- **An editor with Markdown highlighting:** headings are bigger, bold is bold, links, tags, code and formulas are colored. Lists continue on `Enter`, formatting has shortcuts. Split view shows the editor and the result side by side.
- **Completion, like in an IDE:** notes after `[[`, headings after `[[Note#`, images with a preview after `![[`, paths after `](`, tags, callout types, code block languages and attribute colors. `Ctrl+Space` opens it by hand.
- **Emmet-style abbreviations** on `Tab`: `t3x4` is a table, `-*3` a list, `[]*3` tasks, `!tip` a callout, `code:rust` a code block, `flow` a flowchart, `h2{Plan}+[]*3` a section with tasks. There are children `>`, siblings `+`, repeat `*N`, text `{…}`, numbering `$` and groups `( )`. After expanding, `Tab` walks through the input spots, and through cells in tables.
- **Images by drag and drop:** the file is copied into the attachments folder and `![[file.png]]` is inserted into the note.
- **Images of any format open in a tab**, like in Obsidian: PNG, JPEG, WebP, TIFF, TGA, DDS, HDR, EXR, SVG, PSD, JPEG XL and, through the Windows codecs, HEIC, AVIF and camera RAW. The wheel zooms towards the pointer and dragging pans, like Blender's image editor; a double click switches between fit and 100 %. The same formats show inside notes; a double click on an embedded image opens its tab.
- **Hidden folders on request:** the eye in the Files header (or Settings ▸ Notes) shows folders and files whose names start with a dot.

**Reading**
- **A reading view** with callouts (`> [!tip]`), aligned tables, clickable tasks, code highlighting for about 170 languages (GLSL and WGSL included) with tabs on code blocks, images, footnotes, YAML properties and embedded notes `![[…]]`.
- **Terminal sessions and logs:** a `console` block shows a bash session: the `$` prompt is drawn but never copied, “Copy commands” takes only the commands, and Cargo and rustc output is colored like in a terminal. A `log` block highlights timestamps, sources and levels and tints WARN and ERROR lines. Long sessions and logs fold after 12 lines (`lines=N` changes that, `open` turns it off).
- **Diagrams:** `mermaid` blocks (flowcharts, sequence, class, state and pie diagrams) and `dot` (Graphviz) are drawn as diagrams. Parsing and layout are the app's own; export turns them into SVG.
- **LaTeX formulas** with a typesetting engine of its own: fractions, roots, indices, sums and integrals with limits, `\left…\right`, matrices, `cases`, `aligned`, accents, `\mathbb`/`\mathcal`, `\text{…}`, `\color`, `\boxed`.
- **Export to HTML:** a single note into one file with images inside, or the whole vault as a static documentation site with navigation.

**Feels like home (Forma Core)**
- **A layout in the spirit of Blender.** Every area holds one editor, picked from the menu at the left of its header: notes (with their own tabs), files, search, tags, the graph, the outline, backlinks or info. Drag an area corner to split, join or swap areas; drag a header to move an area to another place or a separate OS window.
- **Workspaces** Notes, Graph and Focus, plus your own. The layout is remembered per vault (`.holotype/workspace.json`).
- **A window frame of its own** with the app's top bar instead of the Windows title bar.
- **Themes:** Holotype Dark and Holotype Light by default, Mycelium Dark and Light, Visual Studio Dark as a plugin, and your own `.toml` themes. Note colors have their own keys (`note.*`). An accent color for notes can be set on top of any theme.
- **English and Russian UI,** switchable on the fly. The demo notes are in Russian.
- **Settings in the spirit of JetBrains IDEs:** language, theme, fonts, zoom, a background picture, a keymap where every command can be rebound, plugins and a Notes page. Export and import of settings. **Find Action** (`Ctrl+Shift+A`).
- **Autoscroll like in a browser:** hold the mouse wheel and move the cursor up or down.

### Status

1. ✅ **Notes:** the vault, the editor, the reading view, links, tags, the graph, export.
2. ✅ **Formulas and diagrams:** the LaTeX engine, mermaid and Graphviz.
3. ✅ **IDE-style help:** completion and Emmet-style abbreviations.
4. ✅ **Forma Core:** the shared shell, themes, settings, keymap, plugins, English and Russian.

Not there yet:
- live preview in the editor (as in Obsidian);
- macros (`\def`) in formulas;
- mermaid gantt, ER, mindmap, journey and gitGraph (their code is shown instead);
- the exported site loads KaTeX from a CDN.

### Built with

- [Rust](https://www.rust-lang.org/) and [egui](https://github.com/emilk/egui), rendered through [wgpu](https://wgpu.rs/) on Vulkan.
- The [Forma Core](https://github.com/forma-openworks/forma-core) crates.
- [pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark) for Markdown, [syntect](https://github.com/trishume/syntect) and [two-face](https://github.com/CosmicHorrorDev/two-face) for code highlighting.
- [Fluent](https://projectfluent.org/) for localization.
- [Lucide](https://lucide.dev) icons (ISC License, see `crates/holotype/icons/lucide/LICENSE`).

### Building

You need stable Rust (edition 2024) and a GPU driver with Vulkan support. Development and testing currently happen on Windows.

Cargo downloads the Forma Core crates from GitHub by itself.

```bash
git clone https://github.com/forma-openworks/forma-holotype
cd forma-holotype
cargo run --release
```

Open a vault right away. If the folder doesn't exist, it is created and filled with demo notes:

```bash
cargo run --release -- "D:\Notes"
```

Without an argument, the welcome screen offers to create the demo vault (in Documents) or to open a folder.

Run the tests:

```bash
cargo test
```

The built app is `target\release\forma-holotype.exe`.

### Shortcuts

Every one of them can be changed in **Settings → Keymap**.

| Action | Shortcut |
|---|---|
| New note | `Ctrl+N` |
| Quick open | `Ctrl+O` / `Ctrl+P` |
| Search notes | `Ctrl+Shift+F` |
| Editor ⇄ reading / split | `Ctrl+E` / `Ctrl+Shift+E` |
| Link graph | `Ctrl+G` |
| Daily note | `Ctrl+D` |
| Save all | `Ctrl+S` (notes also save themselves) |
| Close note tab / next note | `Ctrl+W` / `Ctrl+Tab` |
| Back / forward | `Alt+←` / `Alt+→` |
| Rename | `F2` |
| Bold / italic / wiki link | `Ctrl+B` / `Ctrl+I` / `Ctrl+K` |
| Strikethrough / highlight / inline code | `Ctrl+Shift+X` / `Ctrl+Shift+H` / `Ctrl+Shift+C` |
| Expand abbreviation / next spot | `Tab` / `Shift+Tab` |
| Completion | `Ctrl+Space` |
| Settings / Find action / quick switch theme | `Ctrl+Alt+S` / `Ctrl+Shift+A` / ``Ctrl+` `` |
| Zoom | `Ctrl+=` / `Ctrl+-` / `Ctrl+0` |

### Repository layout

| Path | Contents |
|---|---|
| `crates/holotype-vault` | Data without UI: the vault (`vault.rs`), Markdown parsing (`markdown.rs`), the demo vault (`sample.rs`) |
| `crates/holotype` | The app |
| `crates/holotype/src/main.rs` | The app description for the Forma shell (`SPEC`) and startup |
| `crates/holotype/src/app/` | The app inside the shell: commands, open notes, the note view, area editors, menus and status bar, dialogs |
| `crates/holotype/src/panes.rs`, `actions.rs`, `settings.rs` | Area editors, note tabs and built-in workspaces, actions and default shortcuts, the Notes settings |
| `crates/holotype/src/render.rs`, `editor.rs` | The reading view and editor highlighting |
| `crates/holotype/src/emmet.rs`, `complete.rs`, `assist.rs` | Abbreviations, completion and its popup |
| `crates/holotype/src/math.rs`, `diagram/` | The formula engine; mermaid and DOT parsing, graph layout, drawing and SVG export |
| `crates/holotype/src/graph.rs`, `export.rs` | The link graph, export to HTML |
| `crates/holotype/src/theme.rs`, `themes/` | Note color keys (`note.*`), callouts, accents; the built-in Holotype themes |
| `crates/holotype/src/icon.rs` | The app icon from `assets/logo/`, also embedded into the `.exe` |
| `crates/holotype/locales/<lang>/holotype.ftl` | UI translations |
| `crates/holotype/icons/lucide/` | Extra Lucide icons and their license |
| `assets/logo/` | The logo |
| `demo-vault/` | The demo vault, as the app creates it |

### Where data lives

- `%APPDATA%\Forma\Holotype\` stores settings in `options\`, one file per category (`appearance.json`, `keymap.json`, `notes.json`, `vaults.json`…), window placement, the default layout and your own themes in `themes\`. Set `FORMA_CONFIG_HOME` to keep them somewhere else.
- `%LOCALAPPDATA%\Forma\Holotype\` stores plugins.
- `<vault>\.holotype\` belongs to the vault: `workspace.json` is the area layout, `graph.json` the pinned graph nodes.
- Settings of the first versions (`%APPDATA%\Forma Holotype\config.json`) move over by themselves on the first start.

### About

Forma: Holotype is part of FORMA. Idea and design: Holo. The code is written entirely by Claude (Anthropic).

---

## Русский

### Что это

Forma: Holotype — программа для заметок, конспектов и документации в духе Obsidian. Она часть FORMA и построена на [Forma Core](https://github.com/forma-openworks/forma-core): «разные приложения — одна основа». Окно, области, рабочие пространства, темы, настройки, горячие клавиши, «Найти действие», плагины и переводы общие с [Forma Mycelium](https://github.com/forma-openworks/forma-mycelium). Holotype приносит только своё: заметки, редактор, просмотр, граф, формулы, диаграммы и экспорт.

Заметки — обычные `.md` файлы в папке («хранилище»), поэтому их можно открыть в любом редакторе или держать в git.

Голотип — первый экземпляр, по которому описывают новый вид.

### Что уже работает

**Заметки и связи**
- **Хранилище** с деревом папок: создание, переименование (ссылки на заметку обновляются везде), перетаскивание между папками, удаление в корзину.
- **`[[Вики-ссылки]]`**, обратные ссылки, `#теги`, оглавление и сведения о заметке.
- **Граф связей** (`Ctrl+G`) с силовой раскладкой. Перетащенный узел остаётся на месте, а расположение сохраняется в хранилище (`.holotype/graph.json`). Правый щелчок по узлу — открепить, «Сбросить» возвращает исходную раскладку.
- **Быстрый переход** (`Ctrl+O`) — найти или создать заметку по имени, полнотекстовый **поиск** (`Ctrl+Shift+F`), история «назад/вперёд» у каждой вкладки заметки, **ежедневная заметка** (`Ctrl+D`).

**Письмо**
- **Редактор с подсветкой Markdown:** заголовки крупнее, жирный текст жирным, ссылки, теги, код и формулы выделены цветом. Список продолжается по `Enter`, форматирование ставится сочетаниями клавиш. В режиме «Сплит» редактор и результат рядом.
- **Подсказки при вводе, как в IDE:** заметки после `[[`, разделы после `[[Заметка#`, картинки с превью после `![[`, пути в `](`, теги, типы плашек, языки блоков кода и цвета атрибутов. `Ctrl+Space` — вызвать вручную.
- **Сокращения в духе Emmet** по `Tab`: `t3x4` — таблица, `-*3` — список, `[]*3` — задачи, `!tip` — плашка, `code:rust` — блок кода, `flow` — блок-схема, `h2{План}+[]*3` — раздел с задачами. Есть вложение `>`, соседи `+`, повтор `*N`, текст `{…}`, нумерация `$` и группы `( )`. После разворачивания `Tab` ходит по местам для ввода, а в таблицах — по ячейкам.
- **Картинки перетаскиванием:** файл копируется в папку вложений, а в заметку вставляется `![[файл.png]]`.
- **Картинки любых форматов открываются вкладкой**, как в Obsidian: PNG, JPEG, WebP, TIFF, TGA, DDS, HDR, EXR, SVG, PSD, JPEG XL, а через кодеки Windows — HEIC, AVIF и RAW с камер. Колесо приближает к указателю, перетаскивание двигает, как в редакторе изображений Blender; двойной щелчок — «вписать» или 100 %. Те же форматы видны и в заметках; двойной щелчок по картинке в заметке открывает её вкладку.
- **Скрытые папки по желанию:** глаз в заголовке «Файлов» (или «Настройки ▸ Заметки») показывает папки и файлы, чьё имя начинается с точки.

**Чтение**
- **Красивый просмотр:** плашки (`> [!tip]`), таблицы с выравниванием, кликабельные задачи, подсветка кода (около 170 языков, включая GLSL и WGSL) и вкладки у блоков кода, картинки, сноски, YAML-свойства, встраивание заметок `![[…]]`.
- **Консоль и логи:** блок `console` показывает сеанс bash: приглашение `$` нарисовано, но не копируется, «Копировать команды» берёт только команды, а вывод Cargo и rustc раскрашен как в терминале. Блок `log` подсвечивает время, источник и уровень и окрашивает строки WARN и ERROR. Длинные сеансы и логи сворачиваются после 12 строк (`lines=N` меняет порог, `open` отключает).
- **Диаграммы:** блоки `mermaid` (блок-схемы, последовательности, классы, состояния, круговые) и `dot` (Graphviz) рисуются схемами. Разбор и раскладка свои, при экспорте получаются SVG.
- **Формулы LaTeX** на собственном движке вёрстки: дроби, корни, индексы, суммы и интегралы с пределами, `\left…\right`, матрицы, `cases`, `aligned`, акценты, `\mathbb`/`\mathcal`, `\text{…}` по-русски, `\color`, `\boxed`.
- **Экспорт в HTML:** одна заметка в отдельный файл (картинки встраиваются внутрь) или всё хранилище как статический сайт документации с навигацией.

**Как дома (Forma Core)**
- **Раскладка в духе Blender.** В каждой области один редактор, его вид выбирается в меню слева в заголовке: заметки (со своими вкладками), файлы, поиск, теги, граф, оглавление, обратные ссылки или сведения. За уголок области её делят, объединяют с соседней и меняют местами; за заголовок — переносят в другое место или в отдельное окно.
- **Рабочие пространства** «Заметки», «Граф» и «Фокус», плюс свои. Раскладка запоминается для каждого хранилища (`.holotype/workspace.json`).
- **Своя рамка окна:** вместо заголовка Windows — верхняя строка программы.
- **Темы:** по умолчанию Holotype Dark и Holotype Light, ещё Mycelium Dark и Light, Visual Studio Dark из плагина и свои темы в `.toml`. У цветов заметок свои ключи (`note.*`). Поверх любой темы можно задать акцентный цвет заметок.
- **Интерфейс на русском и английском,** язык переключается без перезапуска. Демо-заметки на русском.
- **Настройки в духе JetBrains:** язык, тема, шрифты, масштаб, фоновая картинка, горячие клавиши (любую команду можно переназначить), плагины и страница «Заметки». Экспорт и импорт настроек. **«Найти действие»** (`Ctrl+Shift+A`).
- **Прокрутка как в браузере:** зажать колёсико мыши и вести курсор вверх или вниз.

### Состояние

1. ✅ **Заметки:** хранилище, редактор, просмотр, ссылки, теги, граф, экспорт.
2. ✅ **Формулы и диаграммы:** движок LaTeX, mermaid и Graphviz.
3. ✅ **Помощь как в IDE:** подсказки и сокращения в духе Emmet.
4. ✅ **Forma Core:** общая оболочка, темы, настройки, горячие клавиши, плагины, русский и английский.

Чего пока нет:
- живого просмотра прямо в редакторе (как в Obsidian);
- макросов (`\def`) в формулах;
- диаграмм mermaid gantt, ER, mindmap, journey и gitGraph (вместо них показывается код);
- экспортированный сайт пока подгружает KaTeX из CDN.

### На чём сделано

- [Rust](https://www.rust-lang.org/) и [egui](https://github.com/emilk/egui), отрисовка через [wgpu](https://wgpu.rs/) на Vulkan.
- Крейты [Forma Core](https://github.com/forma-openworks/forma-core).
- [pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark) для Markdown, [syntect](https://github.com/trishume/syntect) и [two-face](https://github.com/CosmicHorrorDev/two-face) для подсветки кода.
- [Fluent](https://projectfluent.org/) для локализации.
- Значки [Lucide](https://lucide.dev) (лицензия ISC, см. `crates/holotype/icons/lucide/LICENSE`).

### Сборка

Нужны Rust (stable, edition 2024) и драйвер видеокарты с поддержкой Vulkan. Разработка и проверка пока идут на Windows.

Крейты Forma Core Cargo скачивает с GitHub сам.

```bash
git clone https://github.com/forma-openworks/forma-holotype
cd forma-holotype
cargo run --release
```

Сразу открыть хранилище. Если папки нет, она будет создана и заполнена демо-заметками:

```bash
cargo run --release -- "D:\Заметки"
```

Без аргумента откроется экран приветствия: можно создать демо-хранилище (в «Документах») или выбрать свою папку.

Тесты:

```bash
cargo test
```

Готовая программа — `target\release\forma-holotype.exe`.

### Горячие клавиши

Любое сочетание можно поменять в **Настройки → Горячие клавиши**.

| Действие | Сочетание |
|---|---|
| Новая заметка | `Ctrl+N` |
| Быстрый переход | `Ctrl+O` / `Ctrl+P` |
| Поиск по заметкам | `Ctrl+Shift+F` |
| Редактор ⇄ чтение / сплит | `Ctrl+E` / `Ctrl+Shift+E` |
| Граф связей | `Ctrl+G` |
| Ежедневная заметка | `Ctrl+D` |
| Сохранить всё | `Ctrl+S` (заметки сохраняются и сами) |
| Закрыть вкладку заметки / следующая заметка | `Ctrl+W` / `Ctrl+Tab` |
| Назад / вперёд | `Alt+←` / `Alt+→` |
| Переименовать | `F2` |
| Жирный / курсив / вики-ссылка | `Ctrl+B` / `Ctrl+I` / `Ctrl+K` |
| Зачёркнутый / выделение / код в строке | `Ctrl+Shift+X` / `Ctrl+Shift+H` / `Ctrl+Shift+C` |
| Развернуть сокращение / следующее поле | `Tab` / `Shift+Tab` |
| Подсказки | `Ctrl+Space` |
| Настройки / найти действие / смена темы | `Ctrl+Alt+S` / `Ctrl+Shift+A` / ``Ctrl+` `` |
| Масштаб | `Ctrl+=` / `Ctrl+-` / `Ctrl+0` |

### Устройство репозитория

| Путь | Что внутри |
|---|---|
| `crates/holotype-vault` | Данные без интерфейса: хранилище (`vault.rs`), разбор Markdown (`markdown.rs`), демо-хранилище (`sample.rs`) |
| `crates/holotype` | Сама программа |
| `crates/holotype/src/main.rs` | Описание программы для оболочки Forma (`SPEC`) и запуск |
| `crates/holotype/src/app/` | Программа внутри оболочки: команды, открытые заметки, просмотр заметки, редакторы областей, меню и строка состояния, диалоги |
| `crates/holotype/src/panes.rs`, `actions.rs`, `settings.rs` | Редакторы областей, вкладки заметок и встроенные рабочие пространства, действия и сочетания по умолчанию, настройки «Заметки» |
| `crates/holotype/src/render.rs`, `editor.rs` | Просмотр заметки и подсветка в редакторе |
| `crates/holotype/src/emmet.rs`, `complete.rs`, `assist.rs` | Сокращения, подсказки и их всплывающий список |
| `crates/holotype/src/math.rs`, `diagram/` | Движок формул; разбор mermaid и DOT, раскладка графов, рисование и выгрузка в SVG |
| `crates/holotype/src/graph.rs`, `export.rs` | Граф связей, экспорт в HTML |
| `crates/holotype/src/theme.rs`, `themes/` | Ключи цветов заметок (`note.*`), плашки, акценты; встроенные темы Holotype |
| `crates/holotype/src/icon.rs` | Иконка программы из `assets/logo/`, её же вшивает в `.exe` сборочный скрипт |
| `crates/holotype/locales/<язык>/holotype.ftl` | Переводы интерфейса |
| `crates/holotype/icons/lucide/` | Дополнительные значки Lucide и их лицензия |
| `assets/logo/` | Логотип |
| `demo-vault/` | Демо-хранилище в том виде, в каком его создаёт программа |

### Где хранятся данные

- `%APPDATA%\Forma\Holotype\` — настройки в `options\`, по файлу на категорию (`appearance.json`, `keymap.json`, `notes.json`, `vaults.json`…), положение окна, стандартная раскладка и свои темы в `themes\`. Переменная `FORMA_CONFIG_HOME` переносит их в другую папку.
- `%LOCALAPPDATA%\Forma\Holotype\` — плагины.
- `<хранилище>\.holotype\` — данные хранилища: `workspace.json` — раскладка областей, `graph.json` — закреплённые узлы графа.
- Настройки первых версий (`%APPDATA%\Forma Holotype\config.json`) при первом запуске переезжают сами.

### О проекте

Forma: Holotype — часть FORMA. Идея и дизайн: Holo. Код полностью написан Claude (Anthropic).
