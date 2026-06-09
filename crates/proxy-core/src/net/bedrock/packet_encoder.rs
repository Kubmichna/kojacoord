use bytes::{BufMut, BytesMut};

/// Encodes a 32-bit integer as an LEB128 VarInt.
pub fn encode_varint(mut value: u32, buf: &mut BytesMut) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        buf.put_u8(byte);
        if value == 0 {
            break;
        }
    }
}

/// Encodes a Bedrock string (VarInt length + bytes).
pub fn encode_string(value: &str, buf: &mut BytesMut) {
    encode_varint(value.len() as u32, buf);
    buf.put_slice(value.as_bytes());
}

/// Encodes a Vec3f.
pub fn encode_vec3f(x: f32, y: f32, z: f32, buf: &mut BytesMut) {
    buf.put_f32_le(x);
    buf.put_f32_le(y);
    buf.put_f32_le(z);
}

/// Wraps payload in a GamePacket (0xFE) and prepends it for RakNet transport.
/// (Standard bedrock format: 0xFE + payload)
pub fn wrap_game_packet(payload: &[u8]) -> BytesMut {
    let mut out = BytesMut::with_capacity(1 + payload.len());
    out.put_u8(0xFE);
    out.put_slice(payload);
    out
}
