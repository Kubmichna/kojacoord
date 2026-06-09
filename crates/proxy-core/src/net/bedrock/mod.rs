pub mod login;
pub mod translator;
pub mod connection;
pub mod packet_decoder;
pub mod packet_encoder;
pub mod chunk_translator;
pub mod block_mapping;

use crate::ProxyState;
use rust_raknet::{RaknetListener, RaknetSocket};
use std::sync::Arc;
use tokio::time::Duration;

pub async fn start_raknet_listener(state: Arc<ProxyState>) -> anyhow::Result<()> {
    let bind_addr = &state.config.bedrock.bind;
    let motd_str = format!("MCPE;{};12;1.20.0;0;{};12345;Kojacoord;Survival;1;19132;19133;", 
        state.config.bedrock.motd,
        state.config.proxy.max_players
    );

    let bind_addr: std::net::SocketAddr = state.config.bedrock.bind.parse().map_err(|e| anyhow::anyhow!("Failed to parse bind addr: {}", e))?;
    let mut listener = RaknetListener::bind(&bind_addr).await.map_err(|e| anyhow::anyhow!("Raknet error: {:?}", e))?;
    
    listener.set_full_motd(motd_str).map_err(|e| anyhow::anyhow!("Raknet error: {:?}", e))?;
    
    tracing::info!("Bedrock (RakNet) listener started on {}", bind_addr);

    listener.listen().await;

    loop {
        if let Ok(mut socket) = listener.accept().await {
            let addr = socket.peer_addr().map_err(|e| anyhow::anyhow!("Raknet error: {:?}", e))?;
            tracing::info!("Bedrock client connected from: {}", addr);
            
            let state_clone = Arc::clone(&state);
            tokio::spawn(async move {
                match login::handle_bedrock_login(&mut socket).await {
                    Ok(session) => {
                        let conn = connection::BedrockConnection::new(socket, state_clone, session);
                        if let Err(e) = conn.run().await {
                            tracing::error!("Bedrock connection ended with error: {}", e);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Bedrock login failed: {}", e);
                    }
                }
            });
        } else {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}
