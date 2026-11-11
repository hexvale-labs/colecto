(module
  (import "colecto" "request" (func $request (param i32 i32 i32 i32) (result i32)))
  (import "colecto" "emit" (func $emit (param i32 i32) (result i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "\7b\22\6f\70\65\72\61\74\69\6f\6e\22\3a\22\61\63\63\6f\75\6e\74\2e\72\65\61\64\22\7d")
  (data (i32.const 512) "\7b\22\74\69\74\6c\65\22\3a\22\43\6f\6e\6e\65\63\74\69\6f\6e\20\70\61\6e\65\6c\22\2c\22\74\65\78\74\22\3a\22\50\65\72\6d\69\73\73\69\6f\6e\20\72\65\71\75\69\72\65\64\3a\20\65\6e\61\62\6c\65\20\61\63\63\6f\75\6e\74\2e\72\65\61\64\20\66\6f\72\20\74\68\69\73\20\61\63\63\6f\75\6e\74\2e\22\2c\22\61\63\74\69\6f\6e\73\22\3a\5b\22\52\65\66\72\65\73\68\22\5d\7d")
  (data (i32.const 1536) "\7b\22\74\69\74\6c\65\22\3a\22\43\6f\6e\6e\65\63\74\69\6f\6e\20\70\61\6e\65\6c\22\2c\22\74\65\78\74\22\3a\22\41\63\63\6f\75\6e\74\20\63\6f\6e\6e\65\63\74\65\64\2e\22\2c\22\61\63\74\69\6f\6e\73\22\3a\5b\22\52\65\66\72\65\73\68\22\5d\7d")
  (data (i32.const 2560) "\7b\22\74\69\74\6c\65\22\3a\22\43\6f\6e\6e\65\63\74\69\6f\6e\20\70\61\6e\65\6c\22\2c\22\74\65\78\74\22\3a\22\41\63\63\6f\75\6e\74\20\64\69\73\63\6f\6e\6e\65\63\74\65\64\2e\22\2c\22\61\63\74\69\6f\6e\73\22\3a\5b\22\52\65\66\72\65\73\68\22\5d\7d")
  (func (export "run")
    i32.const 0 i32.const 28 i32.const 8192 i32.const 4096 call $request drop
    ;; Host responses are canonical JSON. A denied call has an error, not data.
    i32.const 8194 i32.load8_u i32.const 100 i32.eq
    if
      i32.const 8200 i32.load8_u i32.const 116 i32.eq
      if
        i32.const 1536 i32.const 78 call $emit drop
      else
        i32.const 2560 i32.const 81 call $emit drop
      end
    else
      i32.const 512 i32.const 118 call $emit drop
    end))
