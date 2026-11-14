//! Service parts: pure Wasm with no WASI, filesystem, network, process or native IPC imports.
//! Each invocation gets a fresh instance (docs/extensions/ui-model.md, "Service part").
use serde_json::Value;
use wasmi::{
    Caller, Config, Engine, Extern, Linker, Module, Store, StoreLimits, StoreLimitsBuilder,
};

use crate::package::MAX_MODULE;
use std::sync::Arc;
const MAX_OUTPUT: usize = 32 * 1024;
pub const FUEL: u64 = 10_000_000;
pub const MAX_MEMORY: usize = 32 * 1024 * 1024;
type Service = Box<dyn FnMut(Value) -> Result<Value, String> + Send>;
struct Context {
    limits: StoreLimits,
    service: Service,
    action: Vec<u8>,
    output: Option<serde_json::Value>,
    calls: u8,
}
fn engine() -> Engine {
    let mut config = Config::default();
    config.consume_fuel(true);
    Engine::new(&config)
}
fn compile_module(engine: &Engine, bytes: &[u8]) -> Result<Module, String> {
    if bytes.len() > MAX_MODULE || !bytes.starts_with(b"\0asm") {
        return Err("Expected a Wasm binary of at most 16 MiB".into());
    }
    let module = Module::new(engine, bytes).map_err(|e| e.to_string())?;
    for import in module.imports() {
        if import.module() != "colecto" || !["request", "emit", "action"].contains(&import.name()) {
            return Err("Extension imports are restricted to the Colecto ABI".into());
        }
    }
    Ok(module)
}
pub struct Compiled {
    engine: Engine,
    module: Module,
}
pub fn compile(bytes: &[u8]) -> Result<Arc<Compiled>, String> {
    let engine = engine();
    let module = compile_module(&engine, bytes)?;
    Ok(Arc::new(Compiled { engine, module }))
}
#[cfg(test)]
pub fn validate(bytes: &[u8]) -> Result<(), String> {
    compile(bytes).map(|_| ())
}

fn read(caller: &Caller<'_, Context>, ptr: i32, len: i32) -> Option<Vec<u8>> {
    let len = usize::try_from(len).ok()?;
    let ptr = usize::try_from(ptr).ok()?;
    if len > 64 * 1024 {
        return None;
    }
    let memory = caller.get_export("memory")?.into_memory()?;
    let mut bytes = vec![0; len];
    memory.read(caller, ptr, &mut bytes).ok()?;
    Some(bytes)
}
fn write(caller: &mut Caller<'_, Context>, ptr: i32, capacity: i32, bytes: &[u8]) -> i32 {
    let Ok(ptr) = usize::try_from(ptr) else {
        return -1;
    };
    let Ok(capacity) = usize::try_from(capacity) else {
        return -1;
    };
    if bytes.len() > 64 * 1024 || bytes.len() > capacity {
        return -1;
    }
    let Some(Extern::Memory(memory)) = caller.get_export("memory") else {
        return -1;
    };
    if memory.write(caller, ptr, bytes).is_err() {
        return -1;
    }
    bytes.len() as i32
}
/// Run one invocation: `action` is the JSON the guest reads with `colecto.action`, such as
/// `{"event":"bus.request",...}`. Returns what it emitted, or null.
pub fn invoke(compiled: &Compiled, action: &str, service: Service) -> Result<Value, String> {
    if action.len() > 64 * 1024 {
        return Err("Extension action too long".into());
    }
    let engine = &compiled.engine;
    let module = &compiled.module;
    let limits = StoreLimitsBuilder::new()
        .memory_size(MAX_MEMORY)
        .table_elements(1024)
        .memories(1)
        .tables(1)
        .instances(1)
        .trap_on_grow_failure(true)
        .build();
    let mut store = Store::new(
        engine,
        Context {
            limits,
            service,
            action: action.as_bytes().to_vec(),
            output: None,
            calls: 0,
        },
    );
    store.limiter(|data| &mut data.limits);
    store.set_fuel(FUEL).map_err(|e| e.to_string())?;
    let mut linker = Linker::new(engine);
    linker
        .func_wrap(
            "colecto",
            "action",
            |mut caller: Caller<'_, Context>, ptr: i32, capacity: i32| -> i32 {
                let action = caller.data().action.clone();
                write(&mut caller, ptr, capacity, &action)
            },
        )
        .map_err(|e| e.to_string())?;
    linker
        .func_wrap(
            "colecto",
            "emit",
            |mut caller: Caller<'_, Context>, ptr: i32, len: i32| -> i32 {
                if caller.data().output.is_some() {
                    return -1;
                }
                let Some(bytes) = read(&caller, ptr, len) else {
                    return -1;
                };
                if bytes.len() > MAX_OUTPUT {
                    return -1;
                }
                // The only output is a bus answer: {"bus":{"response":...}}.
                let Ok(output) = serde_json::from_slice::<Value>(&bytes) else {
                    return -1;
                };
                if output.as_object().is_none_or(|o| o.len() != 1)
                    || output["bus"]
                        .as_object()
                        .is_none_or(|o| o.len() != 1 || !o.contains_key("response"))
                {
                    return -1;
                }
                caller.data_mut().output = Some(output);
                0
            },
        )
        .map_err(|e| e.to_string())?;
    linker
        .func_wrap(
            "colecto",
            "request",
            |mut caller: Caller<'_, Context>,
             ptr: i32,
             len: i32,
             output: i32,
             capacity: i32|
             -> i32 {
                if caller.data().calls >= 8 {
                    return -1;
                }
                caller.data_mut().calls += 1;
                let Some(bytes) = read(&caller, ptr, len) else {
                    return -1;
                };
                let Ok(request) = serde_json::from_slice::<Value>(&bytes) else {
                    return -1;
                };
                let result = match (caller.data_mut().service)(request) {
                    Ok(data) => serde_json::json!({"ok": true, "data": data}),
                    Err(error) => serde_json::json!({"ok": false, "error": error}),
                };
                let Ok(bytes) = serde_json::to_vec(&result) else {
                    return -1;
                };
                write(&mut caller, output, capacity, &bytes)
            },
        )
        .map_err(|e| e.to_string())?;
    let instance = linker
        .instantiate_and_start(&mut store, module)
        .map_err(|e| e.to_string())?;
    let function = instance
        .get_typed_func::<(), ()>(&store, "run")
        .map_err(|e| e.to_string())?;
    function.call(&mut store, ()).map_err(|e| e.to_string())?;
    Ok(store.data_mut().output.take().unwrap_or(Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn execute(wat: &str) -> Result<Value, String> {
        invoke(
            &*compile(&wat::parse_str(wat).unwrap())?,
            r#"{"event":"bus.request","from":"x","name":"v1/a","payload":null}"#,
            Box::new(|_| Err("Denied".into())),
        )
    }
    fn emitting(output: &str) -> String {
        let escaped: String = output.bytes().map(|b| format!("\\{b:02x}")).collect();
        format!(
            r#"(module
            (import "colecto" "emit" (func $emit (param i32 i32) (result i32)))
            (memory (export "memory") 1) (data (i32.const 0) "{escaped}")
            (func (export "run") i32.const 0 i32.const {} call $emit drop))"#,
            output.len()
        )
    }
    #[test]
    fn rejects_ambient_authority_and_exhaustion() {
        assert!(
            validate(
                &wat::parse_str(r#"(module (import "wasi_snapshot_preview1" "fd_write" (func)))"#)
                    .unwrap()
            )
            .is_err()
        );
        for source in [
            "(module (func (export \"run\") (loop br 0)))",
            "(module (memory (export \"memory\") 513) (func (export \"run\")))",
            "(module (memory (export \"memory\") 1) (func (export \"run\") unreachable))",
        ] {
            assert!(execute(source).is_err());
        }
    }
    #[test]
    fn only_bus_answers_are_accepted_as_output() {
        let source = emitting(r#"{"bus":{"response":"<b>hi</b>"}}"#);
        assert_eq!(execute(&source).unwrap()["bus"]["response"], "<b>hi</b>");
        for other in [
            r#"{"title":"Old panel","text":"","actions":[]}"#,
            r#"{"bus":{}}"#,
            "nope",
        ] {
            assert_eq!(execute(&emitting(other)).unwrap(), Value::Null);
        }
        let grow = source.replace(
            "call $emit drop",
            "call $emit drop i32.const 513 memory.grow drop",
        );
        assert!(execute(&grow).is_err());
        let table = source.replace(
            "(memory (export \"memory\") 1)",
            "(table 1025 funcref) (memory (export \"memory\") 1)",
        );
        assert!(execute(&table).is_err());
    }
    #[test]
    fn host_request_budget_is_enforced_even_if_the_guest_ignores_errors() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let module = wat::parse_str(
            r#"(module
          (import "colecto" "request" (func $request (param i32 i32 i32 i32) (result i32)))
          (memory (export "memory") 1)
          (data (i32.const 0) "{}")
          (func (export "run") (local $count i32)
            (loop $again
              i32.const 0 i32.const 2 i32.const 8192 i32.const 1024 call $request drop
              local.get $count i32.const 1 i32.add local.tee $count
              i32.const 20 i32.lt_u br_if $again)))"#,
        )
        .unwrap();
        let result = invoke(
            &compile(&module).unwrap(),
            "{}",
            Box::new(move |_| {
                observed.fetch_add(1, Ordering::Relaxed);
                Ok(Value::Null)
            }),
        );
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(calls.load(Ordering::Relaxed), 8);
    }
}
