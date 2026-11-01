use crate::config::Budgets;
use crate::structs::descriptor::Descriptor;
use crate::structs::manager::TextureGrant;

/// Works out the texture budget of a mod from its descriptor. A mod that asks for a budget gets what it asks for,
/// up to the manager's per-mod maximum; a mod that asks for none gets the manager's default, also up to that
/// maximum. A request over the maximum is logged.
pub struct TextureGranter {
    default_bytes: u64,
    default_count: usize,
    max_bytes: u64,
    max_count: usize,
}

impl TextureGranter {
    const BYTES_PER_MIB: u64 = 1024 * 1024;

    pub fn new(budgets: &Budgets) -> Self {
        Self {
            default_bytes: budgets.texture_bytes_per_mod,
            default_count: budgets.textures_per_mod,
            max_bytes: budgets.max_texture_bytes_per_mod,
            max_count: budgets.max_textures_per_mod,
        }
    }

    /// The budget of a mod that asks for none: the manager's default, up to the maximum.
    pub fn default_grant(&self) -> TextureGrant {
        let count = self.default_count.min(self.max_count);
        TextureGrant { bytes: self.default_bytes.min(self.max_bytes), count: u32::try_from(count).unwrap_or(u32::MAX) }
    }

    pub fn grant(&self, descriptor: &Descriptor) -> TextureGrant {
        let asked_bytes = descriptor
            .texture_budget_mb
            .map_or(self.default_bytes, |mb| u64::from(mb).saturating_mul(Self::BYTES_PER_MIB));
        let asked_count = descriptor
            .texture_budget_count
            .map_or(self.default_count, |count| usize::try_from(count).unwrap_or(usize::MAX));
        let bytes = asked_bytes.min(self.max_bytes);
        let count = asked_count.min(self.max_count);
        let over_bytes = descriptor.texture_budget_mb.is_some() && asked_bytes > self.max_bytes;
        let over_count = descriptor.texture_budget_count.is_some() && asked_count > self.max_count;
        if over_bytes || over_count {
            curia::info!("texture budget request is over the manager maximum, the maximum is granted", {
                mod_id: descriptor.id().as_str(),
                asked_bytes: asked_bytes,
                asked_count: asked_count,
                granted_bytes: bytes,
                granted_count: count,
            });
        }
        TextureGrant { bytes, count: u32::try_from(count).unwrap_or(u32::MAX) }
    }
}
