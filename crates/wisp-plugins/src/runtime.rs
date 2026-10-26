use crate::{MAX_JSON, MAX_MODULE, Manifest, Request, Response};
use anyhow::{Context, ensure};
use wasmi::{Config, Engine, Linker, Module, Store, StoreLimitsBuilder};

/// Each invocation is a fresh instance. All persistent guest state is explicit
/// JSON. A worker process additionally bounds compilation and wall-clock time.
pub fn invoke(bytes: &[u8], manifest: &Manifest, request: &Request) -> anyhow::Result<Response> {
    manifest.validate()?;
    ensure!(
        bytes.len() <= MAX_MODULE && bytes.starts_with(b"\0asm"),
        "Invalid or oversized Wasm module"
    );
    let input = serde_json::to_vec(request)?;
    ensure!(input.len() <= MAX_JSON, "Plugin input is too large");
    let mut config = Config::default();
    config.consume_fuel(true);
    let engine = Engine::new(&config);
    let module = Module::new(&engine, bytes)?;
    ensure!(
        module.imports().next().is_none(),
        "Plugins cannot import WASI or native functions"
    );
    let limits = StoreLimitsBuilder::new()
        .memory_size(32 * 1024 * 1024)
        .memories(1)
        .tables(2)
        .table_elements(16384)
        .instances(1)
        .build();
    let mut store = Store::new(&engine, limits);
    store.limiter(|limits| limits);
    store.set_fuel(30_000_000)?;
    let instance = Linker::new(&engine).instantiate_and_start(&mut store, &module)?;
    let memory = instance
        .get_memory(&store, "memory")
        .context("Plugin must export memory")?;
    let allocate = instance.get_typed_func::<(), i32>(&store, "wisp_input")?;
    let pointer = usize::try_from(allocate.call(&mut store, ())?)?;
    memory.write(&mut store, pointer, &input)?;
    let handle = instance.get_typed_func::<i32, i64>(&store, "wisp_handle")?;
    let encoded = handle
        .call(&mut store, i32::try_from(input.len())?)?
        .cast_unsigned();
    let offset = usize::try_from(encoded >> 32)?;
    let length = usize::try_from(encoded & 0xffff_ffff)?;
    ensure!(
        length > 0 && length <= MAX_JSON,
        "Plugin output exceeds limits"
    );
    let mut output = vec![0; length];
    memory.read(&store, offset, &mut output)?;
    let response: Response =
        serde_json::from_slice(&output).context("Plugin returned invalid JSON")?;
    response.view.validate(manifest)?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn manifest() -> Manifest {
        Manifest {
            api: 1,
            id: "test.plugin".into(),
            name: "Test".into(),
            version: "1.0.0".into(),
            description: "Test plugin".into(),
            permissions: vec![],
        }
    }
    fn request() -> Request {
        Request {
            api: 1,
            event: "open".into(),
            action: String::new(),
            fields: serde_json::Value::Null,
            state: serde_json::Value::Null,
            document: None,
        }
    }
    #[test]
    fn rejects_wasi_imports() {
        let wasm = wat::parse_str(
            r#"(module (import "wasi_snapshot_preview1" "proc_exit" (func (param i32))))"#,
        )
        .unwrap();
        assert!(
            invoke(&wasm, &manifest(), &request())
                .unwrap_err()
                .to_string()
                .contains("cannot import")
        );
    }
    #[test]
    fn traps_infinite_loop_and_invalid_memory() {
        for code in [
            r#"(module (memory (export "memory") 1) (func (export "wisp_input") (result i32) (loop $forever br $forever) i32.const 0) (func (export "wisp_handle") (param i32) (result i64) i64.const 1))"#,
            r#"(module (memory (export "memory") 1) (func (export "wisp_input") (result i32) i32.const -1) (func (export "wisp_handle") (param i32) (result i64) i64.const 1))"#,
        ] {
            assert!(invoke(&wat::parse_str(code).unwrap(), &manifest(), &request()).is_err());
        }
    }
    #[test]
    fn rejects_oversized_memory() {
        let code = r#"(module (memory (export "memory") 513) (func (export "wisp_input") (result i32) i32.const 0) (func (export "wisp_handle") (param i32) (result i64) i64.const 1))"#;
        assert!(invoke(&wat::parse_str(code).unwrap(), &manifest(), &request()).is_err());
    }
    #[test]
    fn real_valheim_guest_roundtrip_when_built() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/plugins/games.valheim");
        if !root.exists() {
            return;
        }
        let (manifest, wasm, _) = crate::package::Packages::inspect(&root).unwrap();
        let result = invoke(&wasm, &manifest, &request()).unwrap();
        assert_eq!(result.view.title, "Valheim Modpacks");
        assert_eq!(result.view.packages.len(), 1);
        assert!(result.view.document.is_none());
    }
}
