use once_cell::sync::Lazy;
use serde::Serialize;
use std::alloc::{alloc, dealloc, Layout};

#[derive(Serialize)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub min_proxy_version: String,
    pub dependencies: Vec<String>,
}

#[derive(Serialize)]
pub enum PacketDirection {
    Clientbound,
    Serverbound,
}

#[derive(Serialize)]
pub struct PacketEvent {
    pub id: i32,
    pub direction: PacketDirection,
}

#[no_mangle]
pub extern "C" fn allocate(size: usize) -> *mut u8 {
    let layout = Layout::from_size_align(size, 1).unwrap();
    unsafe { alloc(layout) }
}

/// # Safety
/// The caller must ensure that `ptr` points to a valid allocation of at least `size` bytes.
#[no_mangle]
pub unsafe extern "C" fn deallocate(ptr: *mut u8, size: usize) {
    let layout = Layout::from_size_align(size, 1).unwrap();
    dealloc(ptr, layout);
}

static METADATA_JSON: Lazy<String> = Lazy::new(|| {
    let meta = PluginMetadata {
        name: "WasmMockPlugin".to_string(),
        version: "1.0.0".to_string(),
        author: "Antigravity".to_string(),
        description: "A mock WASM plugin.".to_string(),
        min_proxy_version: "0.1.0".to_string(),
        dependencies: vec![],
    };
    serde_json::to_string(&meta).unwrap()
});

#[no_mangle]
pub extern "C" fn get_metadata_ptr() -> *const u8 {
    METADATA_JSON.as_ptr()
}

#[no_mangle]
pub extern "C" fn get_metadata_len() -> usize {
    METADATA_JSON.len()
}

/// # Safety
/// The caller must ensure that `ptr` points to a valid allocation of at least `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn handle_event(ptr: *mut u8, len: usize) -> u64 {
    let _slice = std::slice::from_raw_parts(ptr, len);

    let response_json = r#"{"Message":"Hello from WASM!"}"#;
    let out_len = response_json.len();
    let out_ptr = allocate(out_len);
    unsafe {
        std::ptr::copy_nonoverlapping(response_json.as_ptr(), out_ptr, out_len);
    }

    ((out_ptr as u64) << 32) | (out_len as u64)
}

static HOOKS_JSON: Lazy<String> = Lazy::new(|| {
    let hooks = vec![PacketEvent {
        id: 0x00,
        direction: PacketDirection::Serverbound,
    }];
    serde_json::to_string(&hooks).unwrap()
});

#[no_mangle]
pub extern "C" fn register_packet_hooks() -> u64 {
    let out_len = HOOKS_JSON.len();
    let out_ptr = allocate(out_len);
    unsafe {
        std::ptr::copy_nonoverlapping(HOOKS_JSON.as_ptr(), out_ptr, out_len);
    }
    ((out_ptr as u64) << 32) | (out_len as u64)
}

#[no_mangle]
pub extern "C" fn handle_packet_hook(
    filter_id: i32,
    _packet_id: i32,
    _direction: i32,
    _data_ptr: u32,
    _data_len: u32,
) -> u64 {
    // Return 0 (Forward), 1<<32 (Drop), or a valid pointer<<32 | len for Modify.
    // For test, if filter_id is 0, we Drop (return 1<<32).
    if filter_id == 0 {
        1 << 32 // Drop
    } else {
        0 // Forward
    }
}
