use anyhow::Result;
use bytes::{Buf, BytesMut};

pub fn read_var_u32(buf: &mut BytesMut) -> Result<u32> {
    let mut value = 0;
    let mut shift = 0;
    loop {
        if buf.is_empty() {
            anyhow::bail!("Incomplete var_u32");
        }
        let b = buf.get_u8();
        value |= ((b & 0x7F) as u32) << shift;
        if (b & 0x80) == 0 {
            break;
        }
        shift += 7;
        if shift >= 35 {
            anyhow::bail!("VarInt too big");
        }
    }
    Ok(value)
}

pub fn read_var_u64(buf: &mut BytesMut) -> Result<u64> {
    let mut value = 0;
    let mut shift = 0;
    loop {
        if buf.is_empty() {
            anyhow::bail!("Incomplete var_u64");
        }
        let b = buf.get_u8();
        value |= ((b & 0x7F) as u64) << shift;
        if (b & 0x80) == 0 {
            break;
        }
        shift += 7;
        if shift >= 70 {
            anyhow::bail!("VarLong too big");
        }
    }
    Ok(value)
}

pub fn read_string(buf: &mut BytesMut) -> Result<String> {
    let len = read_var_u32(buf)? as usize;
    if buf.remaining() < len {
        anyhow::bail!("Incomplete string");
    }
    let data = buf.copy_to_bytes(len);
    Ok(String::from_utf8(data.to_vec())?)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Vec3f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub fn read_vec3f(buf: &mut BytesMut) -> Result<Vec3f> {
    if buf.remaining() < 12 {
        anyhow::bail!("Incomplete Vec3f");
    }
    Ok(Vec3f {
        x: buf.get_f32_le(),
        y: buf.get_f32_le(),
        z: buf.get_f32_le(),
    })
}

// ==========================================
// Packet Models
// ==========================================

#[derive(Debug, Clone)]
pub struct MovePlayerPacket {
    pub runtime_id: u64,
    pub position: Vec3f,
    pub pitch: f32,
    pub yaw: f32,
    pub head_yaw: f32,
    pub mode: u8,
    pub on_ground: bool,
    pub riding_runtime_id: u64,
    pub teleportion_cause: Option<i32>,
    pub item_type: Option<i32>,
    pub tick: u64,
}

pub fn decode_move_player(buf: &mut BytesMut) -> Result<MovePlayerPacket> {
    let runtime_id = read_var_u64(buf)?;
    let position = read_vec3f(buf)?;
    
    if buf.remaining() < 13 {
        anyhow::bail!("Incomplete MovePlayerPacket rotation");
    }
    
    let pitch = buf.get_f32_le();
    let yaw = buf.get_f32_le();
    let head_yaw = buf.get_f32_le();
    let mode = buf.get_u8();
    let on_ground = buf.get_u8() != 0;
    let riding_runtime_id = read_var_u64(buf)?;
    
    // Additional fields are optional based on mode, but for standard movement they might be omitted.
    // For exact translation, we'd need to parse teleportation cause if mode == teleport, etc.
    // Stubbing the rest to just extract position/rotation accurately.
    
    Ok(MovePlayerPacket {
        runtime_id,
        position,
        pitch,
        yaw,
        head_yaw,
        mode,
        on_ground,
        riding_runtime_id,
        teleportion_cause: None,
        item_type: None,
        tick: 0,
    })
}

#[derive(Debug, Clone)]
pub struct TextPacket {
    pub type_id: u8,
    pub needs_translation: bool,
    pub source_name: Option<String>,
    pub message: String,
    pub parameters: Vec<String>,
    pub xuid: String,
    pub platform_chat_id: String,
}

pub fn decode_text(buf: &mut BytesMut) -> Result<TextPacket> {
    if buf.remaining() < 2 {
        anyhow::bail!("Incomplete TextPacket header");
    }
    let type_id = buf.get_u8();
    let needs_translation = buf.get_u8() != 0;
    
    let mut source_name = None;
    let mut message = String::new();
    let mut parameters = Vec::new();
    
    match type_id {
        0 => { // Raw
            message = read_string(buf)?;
        }
        1 => { // Chat
            source_name = Some(read_string(buf)?);
            message = read_string(buf)?;
        }
        2 => { // Translation
            message = read_string(buf)?;
            let count = read_var_u32(buf)?;
            for _ in 0..count {
                parameters.push(read_string(buf)?);
            }
        }
        _ => {
            // Other types fallback to just reading a string if possible, or failing
            message = read_string(buf).unwrap_or_default();
        }
    }
    
    let xuid = read_string(buf).unwrap_or_default();
    let platform_chat_id = read_string(buf).unwrap_or_default();

    Ok(TextPacket {
        type_id,
        needs_translation,
        source_name,
        message,
        parameters,
        xuid,
        platform_chat_id,
    })
}
