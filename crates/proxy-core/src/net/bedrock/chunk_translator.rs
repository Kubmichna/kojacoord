use bytes::{Buf, BufMut, BytesMut};
use kojacoord_protocol::types::VarInt;
use kojacoord_protocol::Decode;
use std::collections::HashMap;

/// Decodes a Java 1.12.2 or modern chunk and encodes a Bedrock LevelChunk (0x3A)
pub fn translate_chunk(chunk_x: i32, chunk_z: i32, primary_bit_mask: i32, java_data: &[u8], protocol_version: u32) -> BytesMut {
    let mut java_buf = BytesMut::from(java_data);
    let mut bedrock_payload = BytesMut::new();

    bedrock_payload.put_u8(0x3A);
    crate::net::bedrock::packet_encoder::encode_varint(chunk_x as u32, &mut bedrock_payload);
    crate::net::bedrock::packet_encoder::encode_varint(chunk_z as u32, &mut bedrock_payload);

    let mut subchunk_count = 0;
    for i in 0..16 {
        if (primary_bit_mask & (1 << i)) != 0 {
            subchunk_count += 1;
        }
    }
    crate::net::bedrock::packet_encoder::encode_varint(subchunk_count, &mut bedrock_payload);
    bedrock_payload.put_u8(0);

    let mut chunk_data = BytesMut::new();

    for i in 0..16 {
        if (primary_bit_mask & (1 << i)) != 0 {
            if java_buf.remaining() == 0 { break; }
            let mut bits_per_block = java_buf.get_u8();
            if bits_per_block < 4 { bits_per_block = 4; }

            // Parse Java Palette
            let mut java_palette = Vec::new();
            if bits_per_block <= 8 {
                let palette_len = VarInt::decode(&mut java_buf).unwrap_or(VarInt(0)).0;
                for _ in 0..palette_len {
                    if java_buf.remaining() >= 1 {
                        java_palette.push(VarInt::decode(&mut java_buf).unwrap_or(VarInt(0)).0 as u32);
                    }
                }
            }

            // Parse Data Array
            let data_len = VarInt::decode(&mut java_buf).unwrap_or(VarInt(0)).0;
            let mut u64_data = Vec::with_capacity(data_len as usize);
            for _ in 0..data_len {
                if java_buf.remaining() >= 8 {
                    u64_data.push(java_buf.get_u64());
                } else {
                    u64_data.push(0);
                }
            }

            if java_buf.remaining() >= 2048 { java_buf.advance(2048); }
            if protocol_version <= 340 && java_buf.remaining() >= 2048 { 
                java_buf.advance(2048); // Skip sky light only if present (usually overworld)
            }

            // We must now decode 4096 blocks and build Bedrock palette
            let mut bedrock_blocks = vec![0u32; 4096];
            let mut bedrock_palette = Vec::new();
            let mut palette_map: HashMap<&'static str, u32> = HashMap::new();

            // Always add Air as 0
            bedrock_palette.push("minecraft:air");
            palette_map.insert("minecraft:air", 0);

            let blocks_per_word = 64 / bits_per_block as usize;
            let mask = (1 << bits_per_block) - 1;

            for block_idx in 0..4096 {
                let word_idx = block_idx / blocks_per_word;
                let bit_offset = (block_idx % blocks_per_word) * bits_per_block as usize;
                
                let mut java_state = 0;
                if word_idx < u64_data.len() {
                    let word = u64_data[word_idx];
                    java_state = (word >> bit_offset) & mask;
                }

                // Global ID
                let global_id = if bits_per_block <= 8 {
                    *java_palette.get(java_state as usize).unwrap_or(&0)
                } else {
                    java_state as u32
                };

                let bedrock_name = crate::net::bedrock::block_mapping::get_bedrock_block_name(global_id, protocol_version);
                
                let runtime_id = *palette_map.entry(bedrock_name).or_insert_with(|| {
                    let new_id = bedrock_palette.len() as u32;
                    bedrock_palette.push(bedrock_name);
                    new_id
                });
                
                bedrock_blocks[block_idx] = runtime_id;
            }

            // Encode Bedrock SubChunk Version 8
            chunk_data.put_u8(8);
            chunk_data.put_u8(1); // 1 storage layer

            // Determine Bedrock bits_per_block based on palette size
            let palette_size = bedrock_palette.len();
            let mut bed_bits = 1;
            while (1 << bed_bits) < palette_size { bed_bits += 1; }
            if bed_bits < 1 { bed_bits = 1; }
            if bed_bits == 3 { bed_bits = 4; }
            if bed_bits == 5 || bed_bits == 6 { bed_bits = 6; }
            if bed_bits == 7 { bed_bits = 8; }
            if bed_bits > 8 { bed_bits = 16; }

            chunk_data.put_u8((bed_bits << 1) | 0); // Format

            let bed_blocks_per_word = 32 / bed_bits;
            let bed_words = (4096 + bed_blocks_per_word - 1) / bed_blocks_per_word;

            let mut bed_word_data = vec![0u32; bed_words];
            for block_idx in 0..4096 {
                let word_idx = block_idx / bed_blocks_per_word as usize;
                let bit_offset = (block_idx % bed_blocks_per_word as usize) * bed_bits as usize;
                bed_word_data[word_idx] |= bedrock_blocks[block_idx] << bit_offset;
            }

            for word in bed_word_data {
                chunk_data.put_u32_le(word);
            }

            crate::net::bedrock::packet_encoder::encode_varint(palette_size as u32, &mut chunk_data);
            for i in 0..palette_size {
                crate::net::bedrock::packet_encoder::encode_varint(i as u32, &mut chunk_data); // Use index as mock Runtime ID
            }
        }
    }

    // Append 2D Biome Array (256 bytes). ID 1 is Plains.
    let mut biomes = vec![1u8; 256];
    chunk_data.put_slice(&biomes);

    // Append Border Blocks count (0)
    chunk_data.put_u8(0);

    // Append Block Entities count (VarInt 0 or empty NBT)
    // Actually Bedrock expects an empty NBT compound or just no block entities.
    // If the count is not prefixed, it's just raw NBT tags until end of packet.
    // An empty NBT compound tag is a single byte 0 (TAG_End).
    // Wait, since 1.16, Bedrock doesn't use a count prefix for block entities in chunks, it just reads NBT until the packet ends.
    // We'll write nothing, or a single TAG_End? Let's just leave it empty if no block entities, or write a single 0 byte (TAG_End).
    // Safest is to write nothing, as 0 bytes remaining means 0 block entities.

    crate::net::bedrock::packet_encoder::encode_varint(chunk_data.len() as u32, &mut bedrock_payload);
    bedrock_payload.put_slice(&chunk_data);

    crate::net::bedrock::packet_encoder::wrap_game_packet(&bedrock_payload)
}
