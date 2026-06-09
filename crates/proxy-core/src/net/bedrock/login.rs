use crate::session::{ClientType, ConnectionState, PlayerSession, SharedSession};
use rust_raknet::RaknetSocket;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use base64::{engine::general_purpose, Engine as _};
use serde::Deserialize;
use bytes::{Buf, BytesMut};

#[derive(Deserialize, Debug)]
pub struct BedrockIdentityData {
    #[serde(rename = "XUID")]
    pub xuid: String,
    #[serde(rename = "identity")]
    pub identity: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
}

#[derive(Deserialize, Debug)]
pub struct BedrockClientData {
    #[serde(rename = "ClientRandomId")]
    pub client_random_id: i64,
    #[serde(rename = "DeviceOS")]
    pub device_os: i32,
    #[serde(rename = "DeviceId")]
    pub device_id: String,
    #[serde(rename = "LanguageCode")]
    pub language_code: String,
    #[serde(rename = "GameVersion")]
    pub game_version: String,
}

pub fn decode_jwt_payload<T: serde::de::DeserializeOwned>(jwt: &str) -> anyhow::Result<T> {
    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() != 3 {
        anyhow::bail!("Invalid JWT structure");
    }
    
    let payload_bytes = general_purpose::URL_SAFE_NO_PAD.decode(parts[1])?;
    let payload_str = String::from_utf8(payload_bytes)?;
    let parsed: T = serde_json::from_str(&payload_str)?;
    Ok(parsed)
}

fn read_var_u32(buf: &mut BytesMut) -> anyhow::Result<u32> {
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

fn read_string(buf: &mut BytesMut) -> anyhow::Result<String> {
    let len = read_var_u32(buf)? as usize;
    if buf.remaining() < len {
        anyhow::bail!("Incomplete string");
    }
    let data = buf.copy_to_bytes(len);
    Ok(String::from_utf8(data.to_vec())?)
}

#[derive(Deserialize, Debug)]
pub struct ConnectionRequest {
    pub chain: Vec<String>,
}

pub async fn handle_bedrock_login(
    socket: &mut RaknetSocket,
) -> anyhow::Result<SharedSession> {
    let peer_addr = socket.peer_addr().map_err(|e| anyhow::anyhow!("Raknet error: {:?}", e))?;
    
    // Read the first packet from the client
    let raw_packet = socket.recv().await.map_err(|e| anyhow::anyhow!("Raknet receive error: {:?}", e))?;
    let mut buf = BytesMut::from(raw_packet.as_slice());

    if buf.is_empty() || buf.get_u8() != 0xFE {
        anyhow::bail!("Expected GamePacket 0xFE");
    }

    // The rest is the batched payload. Login is uncompressed.
    let packet_length = read_var_u32(&mut buf)?;
    if buf.remaining() < packet_length as usize {
        anyhow::bail!("Incomplete GamePacket");
    }

    let packet_id = read_var_u32(&mut buf)?;
    if packet_id != 0x01 { // Login
        anyhow::bail!("Expected Login packet, got {}", packet_id);
    }

    let protocol_version = buf.get_i32();
    let request_str = read_string(&mut buf)?;

    // Parse the connection request which is JSON containing the JWT chain
    let request: ConnectionRequest = serde_json::from_str(&request_str)?;

    let mut identity_data: Option<BedrockIdentityData> = None;
    
    for jwt in request.chain {
        if let Ok(data) = decode_jwt_payload::<BedrockIdentityData>(&jwt) {
            identity_data = Some(data);
            break;
        }
    }

    let identity_data = identity_data.ok_or_else(|| anyhow::anyhow!("Missing Identity Data in JWT chain"))?;

    let session = Arc::new(RwLock::new(PlayerSession {
        uuid: Uuid::new_v4(), // In reality we'd parse this from the identity JWT
        username: identity_data.display_name,
        client_type: ClientType::Bedrock { 
            xuid: identity_data.xuid, 
            device_os: "1".to_string() // Hardcoded device_os for now until we parse ClientData
        },
        client_ip: peer_addr.ip(),
        protocol_version: protocol_version as u32,
        state: ConnectionState::Play,
        current_server: None,
        properties: vec![],
        locale: Some("en_US".to_string()),
        view_distance: Some(10),
        rank: "default".to_string(),
    }));

    tracing::info!("Bedrock login sequence completed for {}", peer_addr);

    Ok(session)
}
