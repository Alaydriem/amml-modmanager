use amml_abi::ModId;
use serde::Deserialize;

use crate::structs::descriptor::raw::RawDescriptor;
use crate::structs::descriptor::{Capability, Claim, InputClaim, KeybindDecl, SettingsSection};

/// A mod's registration. `texture_budget_mb` (MiB) and `texture_budget_count` are the texture budget the mod asks
/// for; `None` asks for the manager's default.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(try_from = "RawDescriptor")]
pub struct Descriptor {
    pub namespace: String,
    pub name: String,
    pub display_name: String,
    pub version: String,
    pub author: String,
    pub config_path: String,
    pub texture_budget_mb: Option<u32>,
    pub texture_budget_count: Option<u32>,
    pub claims: Vec<Claim>,
    pub keybinds: Vec<KeybindDecl>,
    pub input_passthrough: Vec<u32>,
    pub settings: Vec<SettingsSection>,
}

impl Descriptor {
    /// The mod id, `@<namespace>/<name>`.
    pub fn id(&self) -> String {
        ModId::format(&self.namespace, &self.name)
    }

    /// The `Input` claim with the descriptor's `input_passthrough` keys, or `None` when the mod claims no input.
    pub fn input_claim(&self) -> Option<InputClaim> {
        let claim = self.claims.iter().find(|claim| claim.capability == Capability::Input)?;
        Some(InputClaim {
            mode: claim.mode,
            block_player_turn: claim.block_player_turn,
            pass_pointer_motion: claim.pass_pointer_motion,
            full_block: claim.full_block,
            passthrough: self.input_passthrough.clone(),
        })
    }
}

impl TryFrom<RawDescriptor> for Descriptor {
    type Error = String;

    fn try_from(raw: RawDescriptor) -> Result<Self, Self::Error> {
        let claims = raw
            .claim
            .into_iter()
            .map(|(name, claim)| {
                let capability = Capability::from_name(&name).ok_or_else(|| format!("unknown capability {name}"))?;
                Ok(claim.into_claim(capability))
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self {
            namespace: raw.namespace,
            name: raw.name,
            display_name: raw.display_name,
            version: raw.version,
            author: raw.author,
            config_path: raw.config_path,
            texture_budget_mb: raw.texture_budget_mb,
            texture_budget_count: raw.texture_budget_count,
            claims,
            keybinds: raw.keybind.into_iter().map(|(action, keybind)| keybind.into_decl(action)).collect(),
            input_passthrough: raw.input_passthrough,
            settings: raw.section.into_iter().map(|(id, section)| section.into_section(id)).collect(),
        })
    }
}
