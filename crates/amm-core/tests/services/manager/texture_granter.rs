use amm_core::config::Budgets;
use amm_core::services::descriptor::DescriptorChecker;
use amm_core::services::manager::TextureGranter;
use amm_core::structs::descriptor::Descriptor;
use amm_core::structs::manager::TextureGrant;

const MIB: u64 = 1024 * 1024;

fn descriptor(extra: &str) -> Descriptor {
    let text = format!(
        "namespace = \"test\"\nname = \"minimap\"\ndisplay_name = \"Minimap\"\nversion = \"0.1.0\"\nauthor = \"A\"\nconfig_path = \"config.hcl\"\n{extra}"
    );
    DescriptorChecker::new(128, 256).parse(&text).unwrap_or_else(|error| panic!("descriptor: {error}"))
}

#[test]
fn a_descriptor_texture_budget_is_granted_up_to_the_manager_maximum() {
    let granter = TextureGranter::new(&Budgets::default());

    let under = granter.grant(&descriptor("texture_budget_mb = 512\ntexture_budget_count = 2200"));
    let over = granter.grant(&descriptor("texture_budget_mb = 4000\ntexture_budget_count = 9000"));
    let small = granter.grant(&descriptor("texture_budget_mb = 8\ntexture_budget_count = 16"));
    let bytes_only = granter.grant(&descriptor("texture_budget_mb = 300"));

    assert_eq!(under, TextureGrant { bytes: 512 * MIB, count: 2200 });
    assert_eq!(over, TextureGrant { bytes: 1024 * MIB, count: 4096 });
    assert_eq!(small, TextureGrant { bytes: 8 * MIB, count: 16 });
    assert_eq!(bytes_only, TextureGrant { bytes: 300 * MIB, count: 1024 });
}

#[test]
fn a_raised_manager_maximum_is_still_capped_by_its_hard_maximum() {
    let raised =
        Budgets { max_texture_bytes_per_mod: u64::MAX, max_textures_per_mod: usize::MAX, ..Budgets::default() };
    let granter = TextureGranter::new(&raised.clamped().0);

    let grant = granter.grant(&descriptor("texture_budget_mb = 4000\ntexture_budget_count = 9000"));

    assert_eq!(grant, TextureGrant { bytes: 2048 * MIB, count: 8128 });
}

#[test]
fn a_mod_without_a_texture_budget_gets_the_defaults() {
    let defaults = TextureGranter::new(&Budgets::default());
    let lowered = Budgets { max_texture_bytes_per_mod: 32 * MIB, max_textures_per_mod: 100, ..Budgets::default() };
    let capped = TextureGranter::new(&lowered);

    let grant = defaults.grant(&descriptor(""));
    let under_a_lower_maximum = capped.grant(&descriptor(""));

    assert_eq!(grant, TextureGrant { bytes: 64 * MIB, count: 1024 });
    assert_eq!(under_a_lower_maximum, TextureGrant { bytes: 32 * MIB, count: 100 });
    assert_eq!((defaults.default_grant(), capped.default_grant()), (grant, under_a_lower_maximum));
}
