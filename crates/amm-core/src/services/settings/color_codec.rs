use hcl::Value;

use crate::services::settings::SchemaChecker;
use crate::structs::descriptor::ColorEncoding;

/// Converts colours between RGBA bytes and the HCL value of each colour encoding.
pub struct ColorCodec;

impl ColorCodec {
    const HEX_PREFIX: char = '#';
    const OPAQUE: u8 = 255;

    /// The colour a value holds, or `None` when the value has the wrong shape or a part is out of range.
    /// A hex string without an alpha pair is opaque.
    pub fn to_rgba(v: &Value, enc: ColorEncoding) -> Option<[u8; 4]> {
        match enc {
            ColorEncoding::PackedRgbaU32 => {
                let packed = u32::try_from(SchemaChecker::integer(v)?).ok()?;
                Some(packed.to_be_bytes())
            }
            ColorEncoding::FloatArray4 => Self::parts(v, |part| {
                let number = part.as_f64().filter(|number| (0.0..=1.0).contains(number))?;
                Some((number * f64::from(u8::MAX)).round() as u8)
            }),
            ColorEncoding::ByteArray4 => Self::parts(v, |part| u8::try_from(SchemaChecker::integer(part)?).ok()),
            ColorEncoding::HexString => Self::hex(v.as_str()?),
        }
    }

    pub fn from_rgba(rgba: [u8; 4], enc: ColorEncoding) -> Value {
        match enc {
            ColorEncoding::PackedRgbaU32 => Value::from(u32::from_be_bytes(rgba)),
            ColorEncoding::FloatArray4 => {
                rgba.iter().map(|byte| Value::from(f64::from(*byte) / f64::from(u8::MAX))).collect()
            }
            ColorEncoding::ByteArray4 => rgba.iter().map(|byte| Value::from(u32::from(*byte))).collect(),
            ColorEncoding::HexString => {
                let [r, g, b, a] = rgba;
                Value::from(format!("{}{r:02X}{g:02X}{b:02X}{a:02X}", Self::HEX_PREFIX))
            }
        }
    }

    fn parts(v: &Value, convert: impl Fn(&Value) -> Option<u8>) -> Option<[u8; 4]> {
        let parts = v.as_array().filter(|parts| parts.len() == 4)?;
        Some([convert(&parts[0])?, convert(&parts[1])?, convert(&parts[2])?, convert(&parts[3])?])
    }

    fn hex(text: &str) -> Option<[u8; 4]> {
        let digits = text.strip_prefix(Self::HEX_PREFIX)?;
        if !(digits.len() == 6 || digits.len() == 8) || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let pair = |index: usize| u8::from_str_radix(digits.get(index * 2..index * 2 + 2)?, 16).ok();
        let alpha = if digits.len() == 8 { pair(3)? } else { Self::OPAQUE };
        Some([pair(0)?, pair(1)?, pair(2)?, alpha])
    }
}
