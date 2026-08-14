use wasmtime::{Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder};

use crate::contract::MAX_VALUE_BYTES;
use crate::verdict::ReasonCode;

const FUEL_LIMIT: u64 = 10_000_000;
const MEMORY_LIMIT: usize = 16 * 1024 * 1024;

pub(crate) struct Sandbox {
    engine: Engine,
}

#[derive(Debug)]
pub(crate) struct SandboxError {
    pub code: ReasonCode,
    pub message: String,
}

struct StoreState {
    limits: StoreLimits,
}

impl Sandbox {
    pub fn new() -> Result<Self, SandboxError> {
        let mut config = Config::new();
        config
            .consume_fuel(true)
            .max_wasm_stack(512 * 1024)
            .cranelift_nan_canonicalization(true)
            .wasm_simd(false)
            .wasm_relaxed_simd(false)
            .wasm_multi_memory(false)
            .wasm_memory64(false)
            .wasm_tail_call(false)
            .wasm_custom_page_sizes(false)
            .wasm_wide_arithmetic(false);

        let engine = Engine::new(&config).map_err(|error| SandboxError {
            code: ReasonCode::ExecutionFailed,
            message: format!("failed to create deterministic Wasmtime engine: {error}"),
        })?;
        Ok(Self { engine })
    }

    pub fn execute(&self, wasm: &[u8], input: &[u8]) -> Result<Vec<u8>, SandboxError> {
        let module = Module::from_binary(&self.engine, wasm).map_err(|error| SandboxError {
            code: ReasonCode::InvalidModule,
            message: format!("computation is not a valid supported WebAssembly module: {error}"),
        })?;

        if let Some(import) = module.imports().next() {
            return Err(SandboxError {
                code: ReasonCode::ForbiddenImport,
                message: format!(
                    "module imports {}::{}, but okf:wasm@1 permits no imports",
                    import.module(),
                    import.name()
                ),
            });
        }

        let state = StoreState {
            limits: StoreLimitsBuilder::new()
                .memory_size(MEMORY_LIMIT)
                .instances(1)
                .memories(1)
                .tables(1)
                .table_elements(10_000)
                .trap_on_grow_failure(true)
                .build(),
        };
        let mut store = Store::new(&self.engine, state);
        store.limiter(|state| &mut state.limits);
        store.set_fuel(FUEL_LIMIT).map_err(|error| SandboxError {
            code: ReasonCode::ExecutionFailed,
            message: format!("failed to set deterministic execution fuel: {error}"),
        })?;

        let linker = Linker::new(&self.engine);
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|error| SandboxError {
                code: ReasonCode::ExecutionFailed,
                message: format!("module instantiation failed: {error}"),
            })?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| abi_error("missing exported memory named `memory`"))?;
        let alloc = instance
            .get_typed_func::<i32, i32>(&mut store, "okf_attestor_alloc")
            .map_err(|_| abi_error("missing export `okf_attestor_alloc: (i32) -> i32`"))?;
        let compute = instance
            .get_typed_func::<(i32, i32), i64>(&mut store, "okf_attestor_compute")
            .map_err(|_| abi_error("missing export `okf_attestor_compute: (i32, i32) -> i64`"))?;

        let input_len = i32::try_from(input.len()).map_err(|_| SandboxError {
            code: ReasonCode::ArtifactTooLarge,
            message: "input is too large for the okf:wasm@1 ABI".to_owned(),
        })?;
        let input_ptr = alloc
            .call(&mut store, input_len)
            .map_err(|error| execution_error("input allocation", error))?;
        if input_ptr < 0 {
            return Err(abi_error("allocator returned a negative input pointer"));
        }
        memory
            .write(&mut store, input_ptr as usize, input)
            .map_err(|error| SandboxError {
                code: ReasonCode::InvalidAbi,
                message: format!("allocator returned an out-of-bounds input region: {error}"),
            })?;

        let packed = compute
            .call(&mut store, (input_ptr, input_len))
            .map_err(|error| execution_error("computation", error))? as u64;
        let output_ptr = (packed >> 32) as u32 as usize;
        let output_len = packed as u32 as usize;
        if output_len > MAX_VALUE_BYTES {
            return Err(SandboxError {
                code: ReasonCode::ArtifactTooLarge,
                message: format!(
                    "module returned {output_len} bytes, exceeding the {MAX_VALUE_BYTES}-byte output limit"
                ),
            });
        }

        let mut output = vec![0; output_len];
        memory
            .read(&store, output_ptr, &mut output)
            .map_err(|error| SandboxError {
                code: ReasonCode::InvalidAbi,
                message: format!("module returned an out-of-bounds output region: {error}"),
            })?;
        Ok(output)
    }
}

fn abi_error(message: impl Into<String>) -> SandboxError {
    SandboxError {
        code: ReasonCode::InvalidAbi,
        message: message.into(),
    }
}

fn execution_error(operation: &str, error: wasmtime::Error) -> SandboxError {
    SandboxError {
        code: ReasonCode::ExecutionFailed,
        message: format!("{operation} failed or exhausted its deterministic limits: {error}"),
    }
}
