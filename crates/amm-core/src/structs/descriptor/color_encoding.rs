use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum ColorEncoding {
    PackedRgbaU32,
    FloatArray4,
    ByteArray4,
    HexString,
}
