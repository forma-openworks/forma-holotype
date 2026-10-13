### Forma: Holotype — notes, study notes and documentation of FORMA.
### Shared strings (File, View, Window and Help menus, the settings window, workspaces)
### come from the Forma platform and shell; only Holotype's own strings live here.

app-tagline =
    Notes, study notes and documentation of FORMA.
    Markdown, wiki links, a graph, formulas and diagrams — all drawn with Vulkan.
app-description = The FORMA knowledge space: notes, study notes and documentation of the studio and the lab. A holotype is the first specimen a new species is described from.

## Menus and actions

menu-go = Go
menu-recent-vaults = Recent vaults
menu-recent-empty = Nothing yet

action-group-note = Notes
action-group-go = Go
action-group-editor = Editor

action-new-note = New note
action-new-folder = New folder
action-daily-note = Daily note
action-save-all = Save all
action-open-vault = Open vault…
action-create-demo = Create demo vault
action-rescan = Refresh file list
action-reveal = Show in Explorer
action-export-note = Export note to HTML…
action-export-site = Export vault as a site…
action-toggle-reading = Editor ⇄ reading
action-split = Split
action-graph = Link graph
action-quick-open = Quick open…
action-search = Search notes
action-back = Back
action-forward = Forward
action-rename-note = Rename note
action-close-note = Close note tab
action-next-note = Next note
action-previous-note = Previous note
action-bold = Bold
action-italic = Italic
action-wiki-link = Wiki link
action-strikethrough = Strikethrough
action-highlight = Highlight
action-inline-code = Inline code
action-completion = Show suggestions
action-shortcuts = Keyboard shortcuts
action-open-new-tab = Open in new tab
action-create = Create
action-delete = Delete

## Tabs and workspaces

mode-edit = Edit
mode-split = Split
mode-preview = Read

pane-files = Files
pane-search = Search
pane-tags = Tags
pane-note = Note
pane-graph = Graph
pane-outline = Outline
pane-backlinks = Backlinks
pane-info = Info

workspace-notes = Notes
workspace-graph = Graph
workspace-focus = Focus

## Welcome screen and status bar

welcome-create-demo = Create demo vault
welcome-open-vault = Open folder as vault…
welcome-recent = Recent

status-notes = { $count ->
    [one] { $count } note
   *[other] { $count } notes
}
status-note-stats = { $words ->
    [one] { $words } word
   *[other] { $words } words
} · { $chars } chars · ~{ $minutes } min read
status-saved = Saved
status-unsaved = Not saved

drop-hint = Drop to insert into the note

## Panes

note-empty-title = No note open
note-empty-hint = Pick a note in the file tree, press { $open } to jump to one, or create a new one.
note-missing-title = Note not found
note-missing-hint = “{ $path }” is no longer in the vault.
note-rename-hint = Double-click to rename
note-new-name = New note
folder-new-name = New folder
editor-hint = Start writing…
panel-no-note = No note open

search-hint = Search notes…  (#tag)
search-empty = Type a word or #tag
search-nothing = Nothing found
tags-empty = No tags yet. Write #tag in a note
outline-empty = No headings
backlinks-empty = No notes link here yet

info-words = Words
info-chars = Characters
info-links = Links
info-modified = Modified
info-tags = Tags

## Dialogs

dialog-new-note = New note
dialog-in-folder = in “{ $folder }”
dialog-new-folder = New folder
dialog-inside-folder = inside “{ $folder }”
dialog-rename = Rename
dialog-rename-hint = Links to this note in other notes are updated automatically.
dialog-delete = Delete?
dialog-delete-hint = “{ $name }” will be moved to the Recycle Bin. You can restore it from there.
dialog-open-vault = Choose a folder for the vault
dialog-export-note = Export note to HTML
dialog-export-site = Folder for the documentation site

quick-open-hint = Find or create a note…
quick-open-create = ✚ Create “{ $name }”
quick-open-empty = No notes yet
quick-open-keys = ↑↓ select · Enter open · Ctrl+Enter in a new tab · Esc close

## Notifications

toast-saved = Saved
toast-save-failed = Couldn't save “{ $name }”: { $error }
toast-open-note-first = Open a note first
toast-vault-create-failed = Couldn't create the vault: { $error }
toast-vault-open-failed = Couldn't open the vault: { $error }
toast-note-create-failed = Couldn't create the note: { $error }
toast-folder-create-failed = Couldn't create the folder: { $error }
toast-demo-failed = Couldn't create the demo: { $error }
toast-move-failed = Couldn't move: { $error }
toast-rename-failed = Couldn't rename: { $error }
toast-delete-failed = Couldn't delete: { $error }
toast-moved-to-trash = Moved to the Recycle Bin
toast-links-updated = { $count ->
    [one] Updated links in { $count } note
   *[other] Updated links in { $count } notes
}
toast-export-done = Done: { $path }
toast-export-failed = Export failed: { $error }
toast-site-done = { $count ->
    [one] Site built: { $count } page in { $path }
   *[other] Site built: { $count } pages in { $path }
}
toast-file-added = File added to “{ $folder }”
toast-file-copied = File copied to attachments
toast-file-add-failed = Couldn't add the file: { $error }

## Vault

vault-untitled = Untitled
vault-default-name = Vault
vault-file = file
vault-attachments = Attachments
vault-error-not-found = Folder not found: { $path }
vault-error-exists = A file or folder with this name already exists
vault-error-exists-in-folder = The folder already has an item with this name
vault-error-empty-path = Empty path
vault-error-into-itself = A folder can't be moved into itself
demo-vault-name = Forma Holotype — demo
daily-folder = Daily
daily-template =
    # { $date }

    ## Tasks

    - [ ]

    ## Notes

## Settings

settings-page-notes = Notes
settings-component-notes = Accent color, text size, line width and how notes open
notes-group-text = Text
notes-group-look = Look
notes-text-size = Text size
notes-readable-width = Readable line width
notes-mono-editor = Monospace font in the editor
notes-default-mode = Open notes in
notes-accent = Accent color
notes-accent-theme = From the theme
notes-accent-hint = The accent color repaints links, selection and buttons on top of the chosen theme.

accent-violet = Violet
accent-blue = Blue
accent-teal = Teal
accent-green = Green
accent-amber = Amber
accent-rose = Rose

## Note preview

preview-properties = Properties
preview-missing-link = This note doesn't exist yet — click to create it
preview-external-image = External image — opens in the browser
preview-open-image = Double-click to open
preview-image-missing = Not found: { $src }
preview-embed-missing = No note “{ $target }”

code-copy = Copy
code-copied = Copied
code-view-code = Code
code-view-diagram = Diagram
code-copy-commands = Copy commands
code-copy-command = Copy command
code-copy-log = Copy log
code-lines = { $count ->
    [one] { $count } line
   *[other] { $count } lines
}
code-show-more = { $count ->
    [one] Show { $count } more line
   *[other] Show { $count } more lines
}
code-collapse = Collapse

callout-abstract = Summary
callout-info = Info
callout-todo = To do
callout-tip = Tip
callout-success = Done
callout-question = Question
callout-warning = Warning
callout-failure = Failure
callout-danger = Danger
callout-bug = Bug
callout-example = Example
callout-quote = Quote
callout-note = Note

## Link graph

graph-title = Link graph
graph-open-note = Open note
graph-create-note = Create note
graph-pin = Pin in place
graph-unpin = Unpin node
graph-reset-layout = Reset layout
graph-orphans = Notes without links
graph-ghosts = Notes not created yet
graph-labels = Labels
graph-fit = Show all
graph-reset = Reset
graph-reset-hint = Move every node back to its starting place
graph-pin-hint = Drag a node and it stays where you drop it
graph-pinned = Pinned nodes: { $count } · right-click to unpin
graph-stats = { $notes ->
    [one] { $notes } note
   *[other] { $notes } notes
} · { $links ->
    [one] { $links } link
   *[other] { $links } links
}
graph-empty = Notes and the links between them will appear here

## Editor suggestions and abbreviations

completion-keys = ↑↓ select · Enter or Tab insert · Esc
completion-preview-empty = The image preview will appear here
completion-notes-count = { $count ->
    [one] { $count } note
   *[other] { $count } notes
}
completion-badge = badge
hint-more-lines = { $count ->
    [one] … and { $count } more line
   *[other] … and { $count } more lines
}

lang-shaders = { $name } shaders
lang-text = Text
lang-mermaid = Mermaid diagram
lang-dot = Graphviz graph
lang-console = Console
lang-log = Log

color-red = red
color-orange = orange
color-yellow = yellow
color-green = green
color-teal = teal
color-cyan = cyan
color-blue = blue
color-purple = purple
color-pink = pink
color-gray = gray

emmet-column = Column { $n }
emmet-badge = badge
emmet-desc-list = List
emmet-desc-task-list = Task list
emmet-desc-ordered-list = Numbered list
emmet-desc-markup = Markup
emmet-desc-items = { $count ->
    [one] { $kind } · { $count } item
   *[other] { $kind } · { $count } items
}
emmet-desc-lines = { $count ->
    [one] { $kind } · { $count } line
   *[other] { $kind } · { $count } lines
}
emmet-desc-heading = Level { $level } heading
emmet-desc-bullet = List item
emmet-desc-ordered = Numbered item
emmet-desc-task = Task
emmet-desc-quote = Quote
emmet-desc-callout = “{ $kind }” callout
emmet-desc-paragraph = Paragraph
emmet-desc-table = { $cols }×{ $rows } table
emmet-desc-code = Code block
emmet-desc-code-lang = { $lang } code block
emmet-desc-math = Formula
emmet-desc-front = Note properties
emmet-desc-rule = Divider
emmet-desc-lorem = { $count ->
    [one] Placeholder text, { $count } word
   *[other] Placeholder text, { $count } words
}
emmet-desc-image = Image from the vault
emmet-desc-image-named = Image { $name }
emmet-desc-link = Link to a note
emmet-desc-kbd = Key
emmet-desc-badge = Badge
emmet-desc-date = Today's date
emmet-desc-time = Current time

emmet-diagram-flow = Mermaid flowchart
emmet-diagram-seq = Sequence diagram
emmet-diagram-class = Class diagram
emmet-diagram-state = State diagram
emmet-diagram-pie = Pie chart
emmet-diagram-dot = Graphviz graph

# Labels of the input spots in diagram templates.
emmet-ph-start = Start
emmet-ph-end = End
emmet-ph-condition = Condition?
emmet-ph-yes = yes
emmet-ph-no = no
emmet-ph-action = Action
emmet-ph-other = Other
emmet-ph-client = Client
emmet-ph-server = Server
emmet-ph-request = Request
emmet-ph-response = Response
emmet-ph-idle = Idle
emmet-ph-working = Working
emmet-ph-go = start
emmet-ph-title = Title
emmet-ph-first = First
emmet-ph-second = Second
emmet-ph-third = Third

## Diagrams

diagram-kind-flowchart = Flowchart
diagram-kind-sequence = Sequence diagram
diagram-kind-class = Class diagram
diagram-kind-state = State diagram
diagram-kind-pie = Pie chart
diagram-kind-dot = Graphviz graph
diagram-unsupported = “{ $kind }” diagrams aren't supported yet — showing the code
diagram-failed = Couldn't draw the diagram: { $error }
diagram-error-empty = empty diagram
diagram-error-no-type = no diagram type given
diagram-error-line = line { $line }: { $error }
diagram-error-unclear = can't make sense of “{ $text }”
diagram-error-unexpected-char = unexpected character “{ $char }”
diagram-error-expected = expected { $what }
diagram-error-expected-graph = expected “digraph” or “graph”
diagram-error-expected-node = expected a node, found “{ $found }”
diagram-error-expected-class = expected a class name
diagram-error-not-found = “{ $what }” not found
diagram-error-attributes = error in the attribute list […]
diagram-error-unclosed-shape = unclosed shape [/ … /]
diagram-error-no-nodes = the diagram has no nodes
diagram-error-no-classes = the diagram has no classes
diagram-error-no-states = the diagram has no states
diagram-error-no-participants = the diagram has no participants
diagram-error-no-values = the pie chart has no values
diagram-error-pie-line = expected “"Name" : number”, found “{ $line }”
diagram-error-not-number = “{ $value }” is not a number
diagram-error-note-text = the note has no text after “:”
diagram-error-note-place = a note must be “right of”, “left of” or “over”
diagram-error-message = the message has no sender or receiver: “{ $line }”

## HTML export

export-backlinks = Links to this note
export-filter = Filter…
export-notes-count = { $count ->
    [one] { $count } note
   *[other] { $count } notes
}
