use std::collections::HashSet;

use crate::errors::DescriptorError;
use crate::services::settings::{HclPath, SchemaChecker};
use crate::structs::descriptor::{Capability, ClaimMode, Descriptor, FieldKind, SettingField};
use crate::structs::settings::ValueFault;

/// Parses a descriptor from HCL and checks it. The limits bound what one mod can register.
pub struct DescriptorChecker {
    max_keybinds: usize,
    max_fields: usize,
}

impl DescriptorChecker {
    const MAX_ID_LEN: usize = 64;
    const CONFIG_EXTENSION: &'static str = ".hcl";
    // the manager's own namespace and name
    const MANAGER_ID: (&'static str, &'static str) = ("amml", "modmanager");
    // Windows opens these as devices whatever the case and whatever follows the first dot
    const DEVICE_NAMES: [&'static str; 4] = ["con", "prn", "aux", "nul"];
    const NUMBERED_DEVICES: [&'static str; 2] = ["com", "lpt"];

    pub fn new(max_keybinds: usize, max_fields: usize) -> Self {
        Self { max_keybinds, max_fields }
    }

    pub fn parse(&self, hcl: &str) -> Result<Descriptor, DescriptorError> {
        Self::check_labels(hcl)?;
        let descriptor: Descriptor = hcl::from_str(hcl).map_err(|error| DescriptorError::Parse(error.to_string()))?;
        self.check(&descriptor)?;
        Ok(descriptor)
    }

    // Labelled blocks become maps when deserialized, so a repeated label must be found in the syntax tree first.
    fn check_labels(text: &str) -> Result<(), DescriptorError> {
        let body = hcl::parse(text).map_err(|error| DescriptorError::Parse(error.to_string()))?;
        let mut claims: HashSet<&str> = HashSet::new();
        let mut actions: HashSet<&str> = HashSet::new();
        let mut sections: HashSet<&str> = HashSet::new();
        for block in body.blocks() {
            let Some(label) = block.labels().first().map(|label| label.as_str()) else {
                continue;
            };
            match block.identifier() {
                "claim" if !claims.insert(label) => {
                    return Err(match Capability::from_name(label) {
                        Some(capability) => DescriptorError::DuplicateClaim(capability),
                        None => DescriptorError::Parse(format!("unknown capability {label}")),
                    });
                }
                "keybind" if !actions.insert(label) => return Err(DescriptorError::DuplicateAction(label.to_string())),
                "section" => {
                    if !sections.insert(label) {
                        return Err(DescriptorError::DuplicateSection(label.to_string()));
                    }
                    Self::check_field_labels(block.body())?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn check_field_labels(section: &hcl::Body) -> Result<(), DescriptorError> {
        let mut paths: HashSet<&str> = HashSet::new();
        for block in section.blocks().filter(|block| block.identifier() == "field") {
            let Some(path) = block.labels().first().map(|label| label.as_str()) else {
                continue;
            };
            if !paths.insert(path) {
                return Err(DescriptorError::DuplicatePath(path.to_string()));
            }
        }
        Ok(())
    }

    pub fn check(&self, descriptor: &Descriptor) -> Result<(), DescriptorError> {
        Self::check_identity(descriptor)?;
        Self::check_texture_budget(descriptor)?;
        self.check_counts(descriptor)?;
        self.check_claims(descriptor)?;
        self.check_fields(descriptor)?;
        self.check_keybinds(descriptor)
    }

    fn check_identity(descriptor: &Descriptor) -> Result<(), DescriptorError> {
        for segment in [&descriptor.namespace, &descriptor.name] {
            Self::check_segment(segment)?;
        }
        let (namespace, name) = (descriptor.namespace.as_str(), descriptor.name.as_str());
        if (namespace, name) == Self::MANAGER_ID || Self::is_device_name(name) {
            return Err(DescriptorError::ReservedName(descriptor.id()));
        }
        let file = &descriptor.config_path;
        let stem_len = file.len().saturating_sub(Self::CONFIG_EXTENSION.len());
        let file_ok = file.ends_with(Self::CONFIG_EXTENSION)
            && stem_len > 0
            && !file.contains("..")
            && file.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
        if !file_ok {
            return Err(DescriptorError::BadConfigPath(file.clone()));
        }
        if Self::is_device_name(file) {
            return Err(DescriptorError::ReservedName(file.clone()));
        }
        Ok(())
    }

    // One half of the id: 1 to 64 bytes of a-z, 0-9 and -.
    fn check_segment(segment: &str) -> Result<(), DescriptorError> {
        if segment.is_empty() {
            return Err(DescriptorError::EmptyId);
        }
        let segment_ok = segment.len() <= Self::MAX_ID_LEN
            && segment.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
        if segment_ok { Ok(()) } else { Err(DescriptorError::BadId(segment.to_string())) }
    }

    // True when the part of `name` before its first dot is a Windows device name: con, prn, aux, nul, or com or
    // lpt with one digit 1 to 9, in any case.
    fn is_device_name(name: &str) -> bool {
        let stem = name.split('.').next().unwrap_or(name).to_ascii_lowercase();
        if Self::DEVICE_NAMES.contains(&stem.as_str()) {
            return true;
        }
        let numbered = |prefix: &str| {
            stem.strip_prefix(prefix)
                .is_some_and(|digit| digit.len() == 1 && matches!(digit.as_bytes()[0], b'1'..=b'9'))
        };
        Self::NUMBERED_DEVICES.iter().any(|prefix| numbered(prefix))
    }

    fn check_texture_budget(descriptor: &Descriptor) -> Result<(), DescriptorError> {
        if descriptor.texture_budget_mb == Some(0) {
            return Err(DescriptorError::ZeroTextureBudget("texture_budget_mb"));
        }
        if descriptor.texture_budget_count == Some(0) {
            return Err(DescriptorError::ZeroTextureBudget("texture_budget_count"));
        }
        Ok(())
    }

    fn check_counts(&self, descriptor: &Descriptor) -> Result<(), DescriptorError> {
        if descriptor.keybinds.len() > self.max_keybinds {
            return Err(DescriptorError::TooManyKeybinds(descriptor.keybinds.len()));
        }
        let fields: usize = descriptor.settings.iter().map(|section| section.fields.len()).sum();
        if fields > self.max_fields {
            return Err(DescriptorError::TooManyFields(fields));
        }
        Ok(())
    }

    fn check_claims(&self, descriptor: &Descriptor) -> Result<(), DescriptorError> {
        let mut seen: HashSet<Capability> = HashSet::new();
        for claim in &descriptor.claims {
            if !seen.insert(claim.capability) {
                return Err(DescriptorError::DuplicateClaim(claim.capability));
            }
            if claim.hide_vanilla_hud && claim.capability != Capability::Hud {
                return Err(DescriptorError::HideHudOnNonHudClaim);
            }
            if claim.block_player_turn && claim.capability != Capability::Input {
                return Err(DescriptorError::BlockTurnOnNonInputClaim);
            }
            if claim.pass_pointer_motion && claim.capability != Capability::Input {
                return Err(DescriptorError::PassMotionOnNonInputClaim);
            }
            if claim.full_block && claim.capability != Capability::Input {
                return Err(DescriptorError::FullBlockOnlyOnInput);
            }
            if claim.full_block && claim.mode != ClaimMode::Exclusive {
                return Err(DescriptorError::FullBlockNeedsExclusive);
            }
            if claim.capability == Capability::RenderSize && claim.mode != ClaimMode::Exclusive {
                return Err(DescriptorError::SharedRenderSize);
            }
        }
        Ok(())
    }

    fn check_fields(&self, descriptor: &Descriptor) -> Result<(), DescriptorError> {
        let mut seen: HashSet<&str> = HashSet::new();
        for field in descriptor.settings.iter().flat_map(|section| &section.fields) {
            Self::check_path(&field.path)?;
            if !seen.insert(field.path.as_str()) {
                return Err(DescriptorError::DuplicatePath(field.path.clone()));
            }
            Self::check_kind(field)?;
            Self::check_default(field)?;
        }
        Ok(())
    }

    fn check_keybinds(&self, descriptor: &Descriptor) -> Result<(), DescriptorError> {
        let mut seen: HashSet<&str> = HashSet::new();
        for keybind in &descriptor.keybinds {
            if !seen.insert(keybind.action.as_str()) {
                return Err(DescriptorError::DuplicateAction(keybind.action.clone()));
            }
            Self::check_path(&keybind.config_path)?;
            let is_chord_field = descriptor
                .settings
                .iter()
                .flat_map(|section| &section.fields)
                .any(|field| field.path == keybind.config_path && field.kind == FieldKind::Chord);
            if !is_chord_field {
                return Err(DescriptorError::KeybindPathNotChord {
                    action: keybind.action.clone(),
                    path: keybind.config_path.clone(),
                });
            }
        }
        Ok(())
    }

    fn check_path(path: &str) -> Result<(), DescriptorError> {
        match HclPath::split(path) {
            Some(_) => Ok(()),
            None => Err(DescriptorError::BadPath(path.to_string())),
        }
    }

    fn check_kind(field: &SettingField) -> Result<(), DescriptorError> {
        let is_valid = match &field.kind {
            FieldKind::Int { min, max, step } => min <= max && *step > 0,
            FieldKind::Float { min, max, step, .. } => min <= max && *step > 0.0,
            FieldKind::Enum { options } => {
                !options.is_empty()
                    && options.iter().enumerate().all(|(i, a)| options[..i].iter().all(|b| a.value != b.value))
            }
            _ => true,
        };
        if is_valid { Ok(()) } else { Err(DescriptorError::BadKind { path: field.path.clone() }) }
    }

    fn check_default(field: &SettingField) -> Result<(), DescriptorError> {
        let path = || field.path.clone();
        match SchemaChecker::fault(field, &field.default) {
            None => Ok(()),
            Some(ValueFault::WrongType) => Err(DescriptorError::DefaultWrongType { path: path() }),
            Some(ValueFault::OutOfRange) => Err(DescriptorError::DefaultOutOfRange { path: path() }),
        }
    }
}
