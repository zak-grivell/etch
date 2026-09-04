; inherits: javascript

(identifier) @variable

((identifier) @keyword
  (#any-of? @keyword "from" "import" "match"))

((identifier) @type
  (#match? @type "^[A-Z]"))
