### Holotype: просмотр заметок (holotype-view) — строки рендера: код, консоль, диаграммы,
### формулы, плашки, блоки плагинов. Программы, которые рисуют заметки, добавляют их к своим.

accent-violet = Фиолетовый
accent-blue = Синий
accent-teal = Бирюзовый
accent-green = Зелёный
accent-amber = Янтарный
accent-rose = Розовый
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
lang-text = Текст
lang-console = Консоль
lang-log = Лог
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
