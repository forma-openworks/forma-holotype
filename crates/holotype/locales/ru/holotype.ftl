### Forma: Holotype — заметки, конспекты и документация FORMA.
### Общие строки (меню «Файл», «Вид», «Окно», «Справка», окно настроек, рабочие
### пространства) приносят платформа и оболочка Forma, здесь — только свои.

app-tagline =
    Заметки, конспекты и документация FORMA.
    Markdown, вики-ссылки, граф, формулы и диаграммы — всё рисуется через Vulkan.
app-description = Пространство знаний FORMA: заметки, конспекты и документация студии и лаборатории. Голотип — первый экземпляр, по которому описывают новый вид.

## Меню и действия

menu-go = Переход
menu-recent-vaults = Недавние хранилища
menu-recent-empty = Пока пусто

action-group-note = Заметки
action-group-go = Переход
action-group-editor = Редактор

action-new-note = Новая заметка
action-new-folder = Новая папка
action-daily-note = Ежедневная заметка
action-save-all = Сохранить всё
action-open-vault = Открыть хранилище…
action-create-demo = Создать демо-хранилище
action-rescan = Обновить список файлов
action-reveal = Показать в проводнике
action-export-note = Экспорт заметки в HTML…
action-export-site = Экспорт хранилища как сайта…
action-toggle-reading = Редактор ⇄ чтение
action-split = Сплит
action-graph = Граф связей
action-quick-open = Быстрый переход…
action-search = Поиск по заметкам
action-back = Назад
action-forward = Вперёд
action-rename-note = Переименовать заметку
action-close-note = Закрыть вкладку заметки
action-next-note = Следующая заметка
action-previous-note = Предыдущая заметка
action-bold = Жирный
action-italic = Курсив
action-wiki-link = Вики-ссылка
action-strikethrough = Зачёркнутый
action-highlight = Выделение маркером
action-inline-code = Код в строке
action-completion = Показать подсказки
action-shortcuts = Горячие клавиши
action-open-new-tab = Открыть в новой вкладке
action-create = Создать
action-delete = Удалить

## Редакторы областей и рабочие пространства

mode-edit = Правка
mode-split = Сплит
mode-preview = Чтение

pane-files = Файлы
pane-search = Поиск
pane-tags = Теги
pane-note = Заметка
pane-graph = Граф
pane-outline = Оглавление
pane-backlinks = Обратные ссылки
pane-info = Сведения
pane-plugin = Панель плагина
kind-group-notes = Заметки
kind-group-navigation = Навигация
kind-group-about = О заметке
kind-group-plugins = Плагины
tab-close-others = Закрыть другие вкладки
tab-close-all = Закрыть все вкладки

workspace-notes = Заметки
workspace-graph = Граф
workspace-focus = Фокус

## Экран приветствия и строка состояния

welcome-create-demo = Создать демо-хранилище
welcome-open-vault = Открыть папку как хранилище…
welcome-recent = Недавние

status-notes = { $count ->
    [one] { $count } заметка
    [few] { $count } заметки
   *[many] { $count } заметок
}
status-note-stats = { $words ->
    [one] { $words } слово
    [few] { $words } слова
   *[many] { $words } слов
} · { $chars } симв. · ~{ $minutes } мин чтения
status-saved = Сохранено
status-unsaved = Не сохранено

drop-hint = Отпусти, чтобы вставить в заметку

## Панели

note-empty-title = Нет открытой заметки
note-empty-hint = Выбери заметку в дереве файлов, нажми { $open } для быстрого перехода или создай новую.
note-missing-title = Заметка не найдена
note-missing-hint = Файла «{ $path }» больше нет в хранилище.
note-rename-hint = Двойной щелчок — переименовать
note-new-name = Новая заметка
folder-new-name = Новая папка
editor-hint = Начни писать…
panel-no-note = Нет открытой заметки

search-hint = Поиск по заметкам…  (#тег)
search-empty = Введи слово или #тег
search-nothing = Ничего не нашлось
tags-empty = Тегов пока нет. Напиши в заметке #тег
outline-empty = Заголовков нет
backlinks-empty = На эту заметку пока никто не ссылается

info-words = Слов
info-chars = Символов
info-links = Ссылок
info-modified = Изменена
info-tags = Теги

## Диалоги

dialog-new-note = Новая заметка
dialog-in-folder = в папке «{ $folder }»
dialog-new-folder = Новая папка
dialog-inside-folder = внутри «{ $folder }»
dialog-rename = Переименовать
dialog-rename-hint = Ссылки на заметку в других заметках обновятся автоматически.
dialog-delete = Удалить?
dialog-delete-hint = «{ $name }» будет перемещён в корзину. Его можно будет восстановить оттуда.
dialog-open-vault = Выбери папку для хранилища
dialog-export-note = Экспорт заметки в HTML
dialog-export-site = Папка для сайта документации

quick-open-hint = Найти или создать заметку…
quick-open-create = ✚ Создать «{ $name }»
quick-open-empty = Заметок пока нет
quick-open-keys = ↑↓ выбор · Enter открыть · Ctrl+Enter в новой вкладке · Esc закрыть

## Уведомления

toast-saved = Сохранено
toast-save-failed = Не удалось сохранить «{ $name }»: { $error }
toast-open-note-first = Сначала открой заметку
toast-vault-create-failed = Не удалось создать хранилище: { $error }
toast-vault-open-failed = Не удалось открыть хранилище: { $error }
toast-note-create-failed = Не удалось создать заметку: { $error }
toast-folder-create-failed = Не удалось создать папку: { $error }
toast-demo-failed = Не удалось создать демо: { $error }
toast-move-failed = Не удалось переместить: { $error }
toast-rename-failed = Не удалось переименовать: { $error }
toast-delete-failed = Не удалось удалить: { $error }
toast-moved-to-trash = Перемещено в корзину
toast-links-updated = { $count ->
    [one] Обновлены ссылки в { $count } заметке
   *[other] Обновлены ссылки в { $count } заметках
}
toast-export-done = Готово: { $path }
toast-export-failed = Ошибка экспорта: { $error }
toast-site-done = { $count ->
    [one] Сайт собран: { $count } страница в { $path }
    [few] Сайт собран: { $count } страницы в { $path }
   *[many] Сайт собран: { $count } страниц в { $path }
}
toast-file-added = Файл добавлен в «{ $folder }»
toast-file-copied = Файл скопирован во вложения
toast-file-add-failed = Не удалось добавить файл: { $error }

## Хранилище

vault-untitled = Без названия
vault-default-name = Хранилище
vault-file = файл
vault-attachments = Вложения
vault-error-not-found = Папка не найдена: { $path }
vault-error-exists = Файл или папка с таким именем уже есть
vault-error-exists-in-folder = В папке уже есть элемент с таким именем
vault-error-empty-path = Пустой путь
vault-error-into-itself = Нельзя переместить папку внутрь самой себя
demo-vault-name = Forma Holotype — демо
daily-folder = Ежедневник
daily-template =
    # { $date }

    ## Задачи

    - [ ]

    ## Заметки

## Настройки

settings-page-notes = Заметки
settings-component-notes = Акцентный цвет, размер текста, ширина строки и вид заметок по умолчанию
notes-group-text = Текст
notes-group-look = Вид
notes-text-size = Размер текста
notes-readable-width = Читаемая ширина строки
notes-mono-editor = Моноширинный шрифт в редакторе
notes-default-mode = Как открывать заметки
notes-accent = Акцентный цвет
notes-accent-theme = Как в теме
notes-accent-hint = Акцентный цвет перекрашивает ссылки, выделение и кнопки поверх выбранной темы.

accent-violet = Фиолетовый
accent-blue = Синий
accent-teal = Бирюзовый
accent-green = Зелёный
accent-amber = Янтарный
accent-rose = Розовый

## Просмотр заметки

preview-properties = Свойства
preview-missing-link = Заметки ещё нет — щёлкни, чтобы создать
preview-external-image = Внешняя картинка — откроется в браузере
preview-open-image = Двойной щелчок — открыть
preview-image-missing = Не найдено: { $src }
preview-embed-missing = Нет заметки «{ $target }»

code-copy = Копировать
code-copied = Скопировано
code-view-code = Код
code-view-diagram = Диаграмма
code-view-block = Блок
menu-open-link = Открыть ссылку
menu-open-link-new-tab = Открыть в новой вкладке
menu-copy-link = Копировать ссылку
code-copy-commands = Копировать команды
code-copy-command = Копировать команду
code-copy-log = Копировать лог
code-lines = { $count ->
    [one] { $count } строка
    [few] { $count } строки
    [many] { $count } строк
   *[other] { $count } строки
}
code-show-more = { $count ->
    [one] Показать ещё { $count } строку
    [few] Показать ещё { $count } строки
    [many] Показать ещё { $count } строк
   *[other] Показать ещё { $count } строки
}
code-collapse = Свернуть

callout-abstract = Кратко
callout-info = Информация
callout-todo = Сделать
callout-tip = Совет
callout-success = Готово
callout-question = Вопрос
callout-warning = Внимание
callout-failure = Ошибка
callout-danger = Опасно
callout-bug = Баг
callout-example = Пример
callout-quote = Цитата
callout-note = Заметка

## Граф связей

graph-title = Граф связей
graph-open-note = Открыть заметку
graph-create-note = Создать заметку
graph-pin = Закрепить на месте
graph-unpin = Открепить узел
graph-reset-layout = Сбросить расположение
graph-orphans = Заметки без связей
graph-ghosts = Несозданные заметки
graph-labels = Подписи
graph-fit = Показать всё
graph-reset = Сбросить
graph-reset-hint = Вернуть все узлы к исходному расположению
graph-pin-hint = Перетащи узел — он останется на месте
graph-pinned = Закреплено узлов: { $count } · правый щелчок — открепить
graph-stats = { $notes ->
    [one] { $notes } заметка
    [few] { $notes } заметки
   *[many] { $notes } заметок
} · { $links ->
    [one] { $links } связь
    [few] { $links } связи
   *[many] { $links } связей
}
graph-empty = Здесь появятся заметки и связи между ними

## Подсказки и сокращения в редакторе

completion-keys = ↑↓ выбор · Enter или Tab — вставить · Esc
completion-preview-empty = Здесь появится превью картинки
completion-notes-count = { $count ->
    [one] { $count } заметка
    [few] { $count } заметки
   *[many] { $count } заметок
}
completion-badge = бейдж
hint-more-lines = { $count ->
    [one] … и ещё { $count } строка
    [few] … и ещё { $count } строки
   *[many] … и ещё { $count } строк
}

lang-shaders = Шейдеры { $name }
lang-text = Текст
lang-mermaid = Диаграмма Mermaid
lang-dot = Граф Graphviz
lang-console = Консоль
lang-log = Лог

color-red = красный
color-orange = оранжевый
color-yellow = жёлтый
color-green = зелёный
color-teal = бирюзовый
color-cyan = голубой
color-blue = синий
color-purple = фиолетовый
color-pink = розовый
color-gray = серый

emmet-column = Колонка { $n }
emmet-badge = бейдж
emmet-desc-list = Список
emmet-desc-task-list = Список задач
emmet-desc-ordered-list = Нумерованный список
emmet-desc-markup = Разметка
emmet-desc-items = { $count ->
    [one] { $kind } · { $count } пункт
    [few] { $kind } · { $count } пункта
   *[many] { $kind } · { $count } пунктов
}
emmet-desc-lines = { $count ->
    [one] { $kind } · { $count } строка
    [few] { $kind } · { $count } строки
   *[many] { $kind } · { $count } строк
}
emmet-desc-heading = Заголовок { $level } уровня
emmet-desc-bullet = Пункт списка
emmet-desc-ordered = Нумерованный пункт
emmet-desc-task = Задача
emmet-desc-quote = Цитата
emmet-desc-callout = Плашка «{ $kind }»
emmet-desc-paragraph = Абзац
emmet-desc-table = Таблица { $cols }×{ $rows }
emmet-desc-code = Блок кода
emmet-desc-code-lang = Блок кода { $lang }
emmet-desc-math = Формула
emmet-desc-front = Свойства заметки
emmet-desc-rule = Линия
emmet-desc-lorem = { $count ->
    [one] Текст-рыба, { $count } слово
    [few] Текст-рыба, { $count } слова
   *[many] Текст-рыба, { $count } слов
}
emmet-desc-image = Картинка из хранилища
emmet-desc-image-named = Картинка { $name }
emmet-desc-link = Ссылка на заметку
emmet-desc-kbd = Клавиша
emmet-desc-badge = Бейдж
emmet-desc-date = Сегодняшняя дата
emmet-desc-time = Текущее время

emmet-diagram-flow = Блок-схема mermaid
emmet-diagram-seq = Диаграмма последовательности
emmet-diagram-class = Диаграмма классов
emmet-diagram-state = Диаграмма состояний
emmet-diagram-pie = Круговая диаграмма
emmet-diagram-dot = Граф Graphviz

# Подписи мест для ввода в заготовках диаграмм.
emmet-ph-start = Начало
emmet-ph-end = Конец
emmet-ph-condition = Условие?
emmet-ph-yes = да
emmet-ph-no = нет
emmet-ph-action = Действие
emmet-ph-other = Другое
emmet-ph-client = Клиент
emmet-ph-server = Сервер
emmet-ph-request = Запрос
emmet-ph-response = Ответ
emmet-ph-idle = Ожидание
emmet-ph-working = Работа
emmet-ph-go = старт
emmet-ph-title = Название
emmet-ph-first = Первое
emmet-ph-second = Второе
emmet-ph-third = Третье

## Диаграммы

diagram-kind-flowchart = Блок-схема
diagram-kind-sequence = Диаграмма последовательности
diagram-kind-class = Диаграмма классов
diagram-kind-state = Диаграмма состояний
diagram-kind-pie = Круговая диаграмма
diagram-kind-dot = Граф Graphviz
diagram-unsupported = Диаграммы «{ $kind }» пока не поддерживаются — показываю код
diagram-failed = Не получилось нарисовать диаграмму: { $error }
diagram-error-empty = пустая диаграмма
diagram-error-no-type = не указан тип диаграммы
diagram-error-line = строка { $line }: { $error }
diagram-error-unclear = непонятная строка «{ $text }»
diagram-error-unexpected-char = неожиданный символ «{ $char }»
diagram-error-expected = ожидалось { $what }
diagram-error-expected-graph = ожидалось «digraph» или «graph»
diagram-error-expected-node = ожидался узел, а встретилось «{ $found }»
diagram-error-expected-class = ожидалось имя класса
diagram-error-not-found = не найдено «{ $what }»
diagram-error-attributes = ошибка в списке атрибутов […]
diagram-error-unclosed-shape = не закрыта фигура [/ … /]
diagram-error-no-nodes = в диаграмме нет ни одного узла
diagram-error-no-classes = в диаграмме нет ни одного класса
diagram-error-no-states = в диаграмме нет ни одного состояния
diagram-error-no-participants = в диаграмме нет участников
diagram-error-no-values = в круговой диаграмме нет ни одного значения
diagram-error-pie-line = ожидалось «"Название" : число», а встретилось «{ $line }»
diagram-error-not-number = «{ $value }» — не число
diagram-error-note-text = у заметки нет текста после «:»
diagram-error-note-place = заметка должна быть «right of», «left of» или «over»
diagram-error-message = у сообщения нет отправителя или получателя: «{ $line }»

## Экспорт в HTML

export-backlinks = Ссылки на эту заметку
export-filter = Фильтр…
export-notes-count = { $count ->
    [one] { $count } заметка
    [few] { $count } заметки
   *[many] { $count } заметок
}
