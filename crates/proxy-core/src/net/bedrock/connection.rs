use crate::session::{ClientType, ConnectionState, SharedSession};
use rust_raknet::RaknetSocket;
use std::sync::Arc;
use crate::ProxyState;

pub struct BedrockConnection {
    socket: RaknetSocket,
    state: Arc<ProxyState>,
    session: SharedSession,
}

use bytes::BytesMut;
use kojacoord_protocol::codec::{Encode, PacketId};
use kojacoord_protocol::types::VarInt;
use kojacoord_protocol::versions::v1_12_2::handshake::ServerboundHandshake;
use kojacoord_protocol::versions::v1_12_2::login::ServerboundLoginStart;
use crate::net::packet_io::write_packet;
use tokio::net::TcpStream;

async fn write_java_packet<T: Encode + PacketId>(
    stream: &mut TcpStream,
    packet: &T,
    protocol_version: u32,
) -> anyhow::Result<()> {
    let pid = T::packet_id(protocol_version) as i32;
    let mut payload = BytesMut::new();
    VarInt(pid).encode(&mut payload)?;
    packet.encode(&mut payload)?;
    write_packet(stream, &payload, -1).await?;
    Ok(())
}

impl BedrockConnection {
    pub fn new(socket: RaknetSocket, state: Arc<ProxyState>, session: SharedSession) -> Self {
        Self { socket, state, session }
    }

    pub async fn run(self) -> Result<(), anyhow::Error> {
        let username = {
            let s = self.session.read().await;
            s.username.clone()
        };
        let _uuid = {
            let s = self.session.read().await;
            s.uuid
        };

        // 1. Evaluate Routing rules to find backend
        let server = self.state.routing.select(&self.state.server_registry)
            .ok_or_else(|| anyhow::anyhow!("No backend server available"))?;
        
        tracing::info!("Routing bedrock player {} to Java backend {}", username, server.name);

        // 2. Connect to Java backend (TCP)
        let mut backend_stream = tokio::net::TcpStream::connect(server.address).await?;
        
        // 3. Send Java Handshake, LoginStart (using kojacoord_protocol)
        let handshake = ServerboundHandshake {
            protocol_version: VarInt(340), // 1.12.2
            server_address: server.address.ip().to_string(),
            server_port: server.address.port(),
            next_state: VarInt(2), // Login
        };
        write_java_packet(&mut backend_stream, &handshake, 340).await?;

        let login_start = ServerboundLoginStart {
            username: username.clone(),
        };
        write_java_packet(&mut backend_stream, &login_start, 340).await?;
        
        tracing::debug!("Sent Java 1.12.2 Handshake & LoginStart to backend for Bedrock player");

        // 4. Enter Translation loop
        crate::net::bedrock::translator::start_translation_loop(self.session.clone(), self.socket, backend_stream).await?;

        Ok(())
    }
}
