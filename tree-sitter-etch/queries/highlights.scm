(comment) @comment
(string) @string
(escape_sequence) @constant.character.escape
(number) @constant.numeric
(boolean) @constant.builtin.boolean
(identifier) @variable
(named_type) @type
(numeric_type) @type.builtin
(parameter (identifier) @variable.parameter)
(object_field name: (identifier) @variable.other.member)
(field_access field: (identifier) @variable.other.member)
(call function: (identifier) @function)
["let" "type" "return" "from" "import" "export" "match" "if"] @keyword
["+" "-" "*" "/" "!" "==" "<" ">" "<=" ">=" "<-" "->" "|" "=" "?"] @operator
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," ";" ":" "."] @punctuation.delimiter
(node) @constant.builtin
