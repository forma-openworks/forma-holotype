### Holotype: просмотр заметок (holotype-view) — строки рендера: код, консоль, диаграммы,
### формулы, плашки, блоки плагинов. Программы, которые рисуют заметки, добавляют их к своим.

accent-violet = Violet
accent-blue = Blue
accent-teal = Teal
accent-green = Green
accent-amber = Amber
accent-rose = Rose
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
code-view-block = Block
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
lang-text = Text
lang-console = Console
lang-log = Log
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
