(comment) @comment.inside
(comment)+ @comment.around

(formals
  ((_) @parameter.inside . ","? @parameter.around) @parameter.around)

(function_expression
  body: (_) @function.inside) @function.around

(binding
  (_) @entry.inside) @entry.around


; What sticky context pins: attribute bindings with a structured value (`outputs = ...`,
; `packages = [ ... ]`), not the lone `{` a lambda starts on.
(binding
  expression: [
    (attrset_expression)
    (rec_attrset_expression)
    (function_expression)
    (apply_expression)
    (let_expression)
    (with_expression)
    (if_expression)
    (binary_expression)
    (list_expression)
  ]) @scope.around
