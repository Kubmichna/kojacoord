use crate::session::SharedSession;
use rust_raknet::RaknetSocket;
use tokio::net::TcpStream;
use bytes::{Buf, BytesMut};
use kojacoord_protocol::versions::v1_12_2::play::{ServerboundMovePlayerPosRot, ServerboundChatMessage, ServerboundTeleportConfirm};
use kojacoord_protocol::codec::{Encode, PacketId};
use kojacoord_protocol::types::VarInt;
use crate::net::packet_io::write_packet;

async fn write_java_packet<W: tokio::io::AsyncWrite + Unpin, T: Encode + PacketId>(
    stream: &mut W,
    packet: &T,
    protocol_version: u32,
    threshold: i32,
) -> anyhow::Result<()> {
    let pid = T::packet_id(protocol_version) as i32;
    let mut payload = BytesMut::new();
    VarInt(pid).encode(&mut payload).map_err(|e| anyhow::anyhow!("Encode error: {:?}", e))?;
    packet.encode(&mut payload).map_err(|e| anyhow::anyhow!("Encode error: {:?}", e))?;
    write_packet(stream, &payload, threshold).await.map_err(|e| anyhow::anyhow!("Write error: {:?}", e))?;
    Ok(())
}

pub async fn start_translation_loop(
    _session: SharedSession,
    mut socket: RaknetSocket,
    mut backend_stream: TcpStream,
) -> anyhow::Result<()> {
    let protocol_version = _session.read().await.protocol_version;
    let (mut backend_read, mut backend_write) = backend_stream.into_split();

    tracing::info!("Translation loop started. (MVP engine active for 1.12.2 duplex)");

    loop {
        tokio::select! {
            // 1. Read from Bedrock client
            raknet_result = socket.recv() => {
                match raknet_result {
                    Ok(packet) => {
                        let mut buf = BytesMut::from(packet.as_slice());
                        if !buf.is_empty() && buf.get_u8() == 0xFE {
                            if buf.remaining() > 0 {
                                let packet_id = buf.get_u8();
                                match packet_id {
                                    0x13 => {
                                        match crate::net::bedrock::packet_decoder::decode_move_player(&mut buf) {
                                            Ok(move_pkt) => {
                                                let java_pkt = ServerboundMovePlayerPosRot {
                                                    x: move_pkt.position.x as f64,
                                                    feet_y: move_pkt.position.y as f64,
                                                    z: move_pkt.position.z as f64,
                                                    yaw: move_pkt.yaw,
                                                    pitch: move_pkt.pitch,
                                                    on_ground: move_pkt.on_ground,
                                                };
                                                if let Err(e) = write_java_packet(&mut backend_write, &java_pkt, 340, -1).await {
                                                    tracing::warn!("Failed sending translated MovePlayer: {}", e);
                                                }
                                            }
                                            Err(e) => tracing::warn!("Failed to decode MovePlayer: {}", e),
                                        }
                                    }
                                    0x09 => {
                                        match crate::net::bedrock::packet_decoder::decode_text(&mut buf) {
                                            Ok(text_pkt) => {
                                                let java_pkt = ServerboundChatMessage {
                                                    message: text_pkt.message,
                                                };
                                                if let Err(e) = write_java_packet(&mut backend_write, &java_pkt, 340, -1).await {
                                                    tracing::warn!("Failed sending translated ChatMessage: {}", e);
                                                }
                                            }
                                            Err(e) => tracing::warn!("Failed to decode TextPacket: {}", e),
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Bedrock RakNet socket error or disconnected: {:?}", e);
                        break;
                    }
                }
            }

            // 2. Read from Java backend
            java_result = crate::net::packet_io::read_packet(&mut backend_read, -1) => {
                match java_result {
                    Ok(mut packet_bytes) => {
                        use kojacoord_protocol::Decode;
                        let packet_id = VarInt::decode(&mut packet_bytes).unwrap_or(VarInt(0)).0;
                        
                        match packet_id {
                            // 1.12.2 KeepAlive (0x1F)
                            0x1F => {
                                // Bedrock doesn't strictly need these, but we MUST reply to Java 
                                // to prevent it from kicking the proxy.
                                use kojacoord_protocol::versions::v1_12_2::play::ServerboundKeepAlive;
                                // We need the KeepAlive ID. The ID in 1.12.2 is an i64.
                                if packet_bytes.remaining() >= 8 {
                                    use bytes::Buf;
                                    let keep_alive_id = packet_bytes.get_i64();
                                    let response = ServerboundKeepAlive { keep_alive_id };
                                    if let Err(e) = write_java_packet(&mut backend_write, &response, 340, -1).await {
                                        tracing::warn!("Failed to bounce Java KeepAlive: {:?}", e);
                                    }
                                }
                            }
                            // 1.12.2 JoinGame (0x23)
                            0x23 => {
                                // Translate to Bedrock StartGame (0x0B)
                                let mut bed_payload = BytesMut::new();
                                bed_payload.put_u8(0x0B); // StartGame
                                crate::net::bedrock::packet_encoder::encode_varint(1, &mut bed_payload); // EntityID
                                crate::net::bedrock::packet_encoder::encode_varint(1, &mut bed_payload); // RuntimeEntityID
                                crate::net::bedrock::packet_encoder::encode_varint(1, &mut bed_payload); // Gamemode (Creative)
                                crate::net::bedrock::packet_encoder::encode_vec3f(0.0, 100.0, 0.0, &mut bed_payload); // Spawn pos
                                crate::net::bedrock::packet_encoder::encode_string("world", &mut bed_payload); // World name

                                let wrapped = crate::net::bedrock::packet_encoder::wrap_game_packet(&bed_payload);
                                if let Err(e) = socket.send(&wrapped, rust_raknet::Reliability::ReliableOrdered).await {
                                    tracing::warn!("Failed sending StartGame: {}", e);
                                }
                            }
                            // 1.12.2 PlayerPositionAndLook (0x2F)
                            0x2F => {
                                // Translate to Bedrock MovePlayer (0x13)
                                // We need x, y, z from Java payload
                                if packet_bytes.remaining() >= 32 {
                                    use bytes::Buf;
                                    let x = packet_bytes.get_f64() as f32;
                                    let y = packet_bytes.get_f64() as f32;
                                    let z = packet_bytes.get_f64() as f32;
                                    let yaw = packet_bytes.get_f32();
                                    let pitch = packet_bytes.get_f32();

                                    // Parse flags and teleport ID to send TeleportConfirm back to Java!
                                    if packet_bytes.remaining() >= 1 {
                                        let _flags = packet_bytes.get_u8();
                                        use kojacoord_protocol::Decode;
                                        
                                        // Convert remaining bytes into a `Bytes` struct because `Decode` might require it.
                                        let mut frozen = packet_bytes.split().freeze();
                                        if let Ok(teleport_id) = VarInt::decode(&mut frozen) {
                                            let confirm_pkt = ServerboundTeleportConfirm { teleport_id };
                                            if let Err(e) = write_java_packet(&mut backend_write, &confirm_pkt, 340, -1).await {
                                                tracing::warn!("Failed to send TeleportConfirm: {}", e);
                                            }
                                        }
                                    }

                                    let mut bed_payload = BytesMut::new();
                                    bed_payload.put_u8(0x13); // MovePlayer
                                    crate::net::bedrock::packet_encoder::encode_varint(1, &mut bed_payload); // RuntimeEntityID
                                    crate::net::bedrock::packet_encoder::encode_vec3f(x, y, z, &mut bed_payload);
                                    bed_payload.put_f32_le(pitch);
                                    bed_payload.put_f32_le(yaw);
                                    bed_payload.put_f32_le(yaw); // Head yaw
                                    bed_payload.put_u8(1); // Mode (Teleport = 1)
                                    bed_payload.put_u8(1); // OnGround
                                    crate::net::bedrock::packet_encoder::encode_varint(0, &mut bed_payload); // RiddenEntityID
                                    bed_payload.put_i32_le(0); // TeleportCause (Unknown = 0)
                                    bed_payload.put_i32_le(0); // ItemType

                                    let wrapped = crate::net::bedrock::packet_encoder::wrap_game_packet(&bed_payload);
                                    let _ = socket.send(&wrapped, rust_raknet::Reliability::ReliableOrdered).await;
                                }
                            }
                            // 1.12.2 ChatMessage (0x0F)
                            0x0F => {
                                // String decode
                                let json_msg = kojacoord_protocol::types::StringProto::decode(&mut packet_bytes)
                                    .unwrap_or(kojacoord_protocol::types::StringProto(String::new())).0;
                                
                                let mut bed_payload = BytesMut::new();
                                bed_payload.put_u8(0x09); // TextPacket
                                bed_payload.put_u8(1); // Type (Chat)
                                bed_payload.put_u8(0); // Needs translation
                                crate::net::bedrock::packet_encoder::encode_string("[Server]", &mut bed_payload);
                                crate::net::bedrock::packet_encoder::encode_string(&json_msg, &mut bed_payload);

                                let wrapped = crate::net::bedrock::packet_encoder::wrap_game_packet(&bed_payload);
                                let _ = socket.send(&wrapped, rust_raknet::Reliability::ReliableOrdered).await;
                            }
                            // 1.12.2 ChunkData (0x20)
                            0x20 => {
                                use kojacoord_protocol::versions::v1_12_2::play::ClientboundLevelChunkWithLight;
                                if let Ok(java_chunk) = ClientboundLevelChunkWithLight::decode(&mut packet_bytes) {
                                    let bedrock_chunk = crate::net::bedrock::chunk_translator::translate_chunk(
                                        java_chunk.chunk_x,
                                        java_chunk.chunk_z,
                                        java_chunk.primary_bit_mask.0,
                                        &java_chunk.data,
                                        protocol_version
                                    );
                                    let _ = socket.send(&bedrock_chunk, rust_raknet::Reliability::ReliableOrdered).await;
                                }
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Java backend socket error or disconnected: {:?}", e);
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}
