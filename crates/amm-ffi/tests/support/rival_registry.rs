use std::cell::Cell;
use std::ffi::c_void;

use amml_abi::{AmmlApi, AmmlRegistryTable, AmmlServices, AmmlStatus, AmmlStr};
use amml_api::testing::FakeGame;
use amml_api::{Ctx, Uses};
use amml_runtime::ProviderRegistry;

thread_local! {
    static OWNER: Cell<usize> = const { Cell::new(0) };
    static REGISTRY: ProviderRegistry<usize> = ProviderRegistry::new();
}

/// The loader table of a `FakeGame` with its registry swapped for the loader's own provider registry, so two mods
/// (owners) can provide: the fake has one owner only. `ctx_as` gives a `Ctx` that provides and requires as one owner.
pub struct RivalRegistry {
    api: Box<AmmlApi>,
    _services: Box<AmmlServices>,
    _registry: Box<AmmlRegistryTable>,
}

impl RivalRegistry {
    pub fn new(fake: &FakeGame) -> RivalRegistry {
        // SAFETY: the fake's table is a full v3 table that lives as long as the fake
        let mut api = Box::new(unsafe { *fake.api_table() });
        // SAFETY: a v3 table points to a services table that lives as long as the fake
        let mut services = Box::new(unsafe { *api.services });
        let registry = Box::new(AmmlRegistryTable {
            size: size_of::<AmmlRegistryTable>() as u32,
            version: 1,
            provide: Self::provide,
            require: Self::require,
        });
        services.registry = &raw const *registry;
        api.services = &raw const *services;
        RivalRegistry { api, _services: services, _registry: registry }
    }

    /// A `Ctx` whose registry calls run as mod `owner`.
    pub fn ctx_as(&self, owner: usize) -> Ctx {
        OWNER.with(|cell| cell.set(owner));
        // SAFETY: the table is a full v3 table owned by `self`
        unsafe { Ctx::from_raw(&raw const *self.api, "@test/rival", Uses::NONE) }
            .unwrap_or_else(|error| panic!("the rival table was refused: {error}"))
    }

    unsafe extern "C" fn provide(name: AmmlStr, major: u16, minor: u16, table: *const c_void) -> AmmlStatus {
        // SAFETY: the registry passes a name valid for the call
        let Ok(name) = (unsafe { name.as_str() }) else {
            return AmmlStatus::INVALID_ARGUMENT;
        };
        let owner = OWNER.with(Cell::get);
        match REGISTRY.with(|registry| registry.provide(name, major, minor, owner, table.expose_provenance())) {
            Ok(()) => AmmlStatus::OK,
            Err(_) => AmmlStatus::REFUSED,
        }
    }

    unsafe extern "C" fn require(
        name: AmmlStr,
        major: u16,
        min_minor: u16,
        out_table: *mut *const c_void,
        out_minor: *mut u16,
    ) -> AmmlStatus {
        // SAFETY: the registry passes a name valid for the call
        let Ok(name) = (unsafe { name.as_str() }) else {
            return AmmlStatus::INVALID_ARGUMENT;
        };
        let Some((table, minor)) = REGISTRY.with(|registry| registry.require(name, major, min_minor)) else {
            return AmmlStatus::NOT_AVAILABLE;
        };
        // SAFETY: the registry passes two live out pointers for the call
        unsafe {
            out_table.write(std::ptr::with_exposed_provenance(table));
            out_minor.write(minor);
        }
        AmmlStatus::OK
    }
}
