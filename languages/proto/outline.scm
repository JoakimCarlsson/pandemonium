; Protocol Buffers: the items an outline lists. @item is the whole item, @name
; what it is called and @context the words that say what it is.

(message
  "message" @context
  (message_name) @name) @item

(enum
  "enum" @context
  (enum_name) @name) @item

(service
  "service" @context
  (service_name) @name) @item

(rpc
  "rpc" @context
  (rpc_name) @name) @item

(oneof
  "oneof" @context
  (identifier) @name) @item

(extend
  "extend" @context
  (full_ident) @name) @item

(group
  "group" @context
  (message_name) @name) @item
