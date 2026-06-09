pub mod v1_12_2;
pub mod v1_16_5;
pub mod v1_20_4;

pub fn get_bedrock_block_name(java_id: u32, protocol_version: u32) -> &'static str {
    match protocol_version {
        v if v <= 340 => v1_12_2::map_legacy_v1_12_2(java_id as u16),
        v if v <= 754 => v1_16_5::map_modern_v1_16_5(java_id),
        v if v <= 763 => v1_20_4::map_modern_v1_20_4(java_id),
        _ => v1_12_2::map_legacy_v1_12_2(java_id as u16), // Fallback
    }
}
