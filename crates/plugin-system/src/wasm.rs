use anyhow::{Context, Result};
use wasmtime::{Engine, Store, Instance, Module, Memory, TypedFunc, Linker};
use crate::api::{Plugin, PluginContext, PluginEvent, PluginResponse, PluginMetadata, PacketEvent};
use std::sync::{Arc, Mutex};
use crate::api::{PacketFilter, PacketHookResult, PacketData};

pub struct WasmState {
    pub store: Store<()>,
    pub memory: Memory,
    pub allocate: TypedFunc<u32, u32>,
    pub deallocate: TypedFunc<(u32, u32), ()>,
    pub handle_event_func: TypedFunc<(u32, u32), u64>,
    pub register_packet_hooks_func: Option<TypedFunc<(), u64>>,
    pub handle_packet_hook_func: Option<TypedFunc<(i32, i32, u32, u32, u32), u64>>,
}

pub struct WasmPlugin {
    state: Arc<Mutex<WasmState>>,
    metadata: PluginMetadata,
}

impl WasmState {
    fn write_to_memory(&mut self, data: &[u8]) -> Result<(u32, u32)> {
        let size = data.len() as u32;
        let ptr = self.allocate.call(&mut self.store, size)?;
        
        let mem = self.memory.data_mut(&mut self.store);
        let dest = &mut mem[ptr as usize..(ptr + size) as usize];
        dest.copy_from_slice(data);
        
        Ok((ptr, size))
    }

    fn read_from_memory(&self, ptr: u32, size: u32) -> Result<Vec<u8>> {
        let mem = self.memory.data(&self.store);
        let src = &mem[ptr as usize..(ptr + size) as usize];
        Ok(src.to_vec())
    }
}

impl WasmPlugin {
    pub fn new(engine: &Engine, module: &Module, metadata: PluginMetadata) -> Result<Self> {
        let mut store = Store::new(engine, ());
        let linker = Linker::new(engine);

        let instance = linker.instantiate(&mut store, module)?;
        let memory = instance
            .get_memory(&mut store, "memory")
            .context("WASM module must export a 'memory'")?;

        let allocate = instance.get_typed_func::<u32, u32>(&mut store, "allocate")?;
        let deallocate = instance.get_typed_func::<(u32, u32), ()>(&mut store, "deallocate")?;
        let handle_event_func = instance.get_typed_func::<(u32, u32), u64>(&mut store, "handle_event")?;
        
        let register_packet_hooks_func = instance.get_typed_func::<(), u64>(&mut store, "register_packet_hooks").ok();
        let handle_packet_hook_func = instance.get_typed_func::<(i32, i32, u32, u32, u32), u64>(&mut store, "handle_packet_hook").ok();

        let state = WasmState {
            store,
            memory,
            allocate,
            deallocate,
            handle_event_func,
            register_packet_hooks_func,
            handle_packet_hook_func,
        };

        Ok(Self {
            state: Arc::new(Mutex::new(state)),
            metadata,
        })
    }
}

impl Plugin for WasmPlugin {
    fn name(&self) -> &str { &self.metadata.name }
    fn version(&self) -> &str { &self.metadata.version }
    fn author(&self) -> &str { &self.metadata.author }
    fn description(&self) -> &str { &self.metadata.description }

    fn on_load(&mut self, _context: &PluginContext) -> Result<()> {
        Ok(())
    }

    fn on_unload(&mut self) -> Result<()> { Ok(()) }
    fn on_enable(&mut self) -> Result<()> { Ok(()) }
    fn on_disable(&mut self) -> Result<()> { Ok(()) }

    fn handle_event(&mut self, event: &PluginEvent) -> Result<Option<PluginResponse>> {
        let json = serde_json::to_vec(event)?;
        let mut state = self.state.lock().unwrap();

        let (ptr, len) = state.write_to_memory(&json)?;

        let func = state.handle_event_func.clone();
        let result = match func.call(&mut state.store, (ptr, len)) {
            Ok(v) => v,
            Err(e) => {
                let dealloc = state.deallocate.clone();
                let _ = dealloc.call(&mut state.store, (ptr, len));
                return Err(e.into());
            }
        };

        let ret_ptr = (result >> 32) as u32;
        let ret_len = (result & 0xFFFFFFFF) as u32;

        if ret_ptr == 0 || ret_len == 0 {
            let dealloc = state.deallocate.clone();
            let _ = dealloc.call(&mut state.store, (ptr, len));
            return Ok(None);
        }

        let resp_data = state.read_from_memory(ret_ptr, ret_len)?;
        let response_res: Result<PluginResponse, _> = serde_json::from_slice(&resp_data);
        
        let dealloc = state.deallocate.clone();
        let _ = dealloc.call(&mut state.store, (ptr, len));
        let _ = dealloc.call(&mut state.store, (ret_ptr, ret_len));

        Ok(Some(response_res?))
    }

    fn register_packet_hooks(&mut self) -> Vec<PacketEvent> {
        let mut state = self.state.lock().unwrap();
        
        let func = match state.register_packet_hooks_func.clone() {
            Some(f) => f,
            None => return Vec::new(),
        };
        
        let result = match func.call(&mut state.store, ()) {
            Ok(v) => v,
            Err(e) => {
                log::error!("Failed to call register_packet_hooks in WASM plugin: {:?}", e);
                return Vec::new();
            }
        };
        
        let ptr = (result >> 32) as u32;
        let len = (result & 0xFFFFFFFF) as u32;
        
        if ptr == 0 || len == 0 {
            return Vec::new();
        }
        
        let hooks_res = state.read_from_memory(ptr, len).and_then(|data| {
            serde_json::from_slice::<Vec<PacketFilter>>(&data).map_err(Into::into)
        });
        
        let dealloc = state.deallocate.clone();
        let _ = dealloc.call(&mut state.store, (ptr, len));
        
        let filters = match hooks_res {
            Ok(hooks) => hooks,
            Err(e) => {
                log::error!("Failed to parse packet hooks from WASM plugin: {:?}", e);
                return Vec::new();
            }
        };

        // We drop the lock here so the closures can lock it themselves when invoked
        drop(state);

        let mut events = Vec::new();
        for (i, filter) in filters.into_iter().enumerate() {
            let state_clone = Arc::clone(&self.state);
            let filter_id = i as i32;
            
            let hook_fn = Box::new(move |packet_data: &PacketData| -> anyhow::Result<PacketHookResult> {
                let mut state = state_clone.lock().unwrap();
                let func = match state.handle_packet_hook_func.clone() {
                    Some(f) => f,
                    None => return Ok(PacketHookResult::Forward),
                };
                
                let dir_val = match packet_data.direction {
                    crate::api::PacketDirection::Clientbound => 0,
                    crate::api::PacketDirection::Serverbound => 1,
                };
                
                // Write the raw packet data bytes directly to WASM memory
                let (ptr, len) = state.write_to_memory(&packet_data.data)?;
                
                let result = match func.call(&mut state.store, (filter_id, packet_data.packet_id, dir_val, ptr, len)) {
                    Ok(v) => v,
                    Err(e) => {
                        let dealloc = state.deallocate.clone();
                        let _ = dealloc.call(&mut state.store, (ptr, len));
                        return Err(e.into());
                    }
                };
                
                let ret_ptr = (result >> 32) as u32;
                let ret_len = (result & 0xFFFFFFFF) as u32;
                
                // Free the input bytes
                let dealloc = state.deallocate.clone();
                let _ = dealloc.call(&mut state.store, (ptr, len));
                
                if ret_ptr == 0 && ret_len == 0 {
                    return Ok(PacketHookResult::Forward);
                } else if ret_ptr == 1 && ret_len == 0 {
                    return Ok(PacketHookResult::Drop);
                } else {
                    // Read the modified bytes
                    let resp_data = state.read_from_memory(ret_ptr, ret_len)?;
                    let dealloc = state.deallocate.clone();
                    let _ = dealloc.call(&mut state.store, (ret_ptr, ret_len));
                    return Ok(PacketHookResult::Modify(bytes::Bytes::from(resp_data)));
                }
            });
            
            events.push(PacketEvent::hook(filter, hook_fn));
        }
        
        events
    }
}
