use std::cell::Cell;

use amml_modmanager::errors::DupError;
use amml_modmanager::{DupGuard, ManagerMod};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

use super::support::manager_dll::ManagerDll;

#[test]
fn the_copy_mods_find_passes() {
    let dll = ManagerDll::load_copy();
    assert!(DupGuard::check(dll.module).is_ok());
}

#[test]
fn a_second_copy_from_another_folder_names_the_first_and_stays_off() {
    let first = ManagerDll::load_copy();
    let second = ManagerDll::load_copy();
    assert_ne!(first.module, second.module);

    let error = DupGuard::check(second.module).unwrap_err();

    let DupError::OtherCopy { path } = &error else { panic!("expected OtherCopy, got {error:?}") };
    assert_eq!(std::fs::canonicalize(path).unwrap(), std::fs::canonicalize(&first.path).unwrap());
    let line = error.to_string();
    assert!(
        line.starts_with("another manager (amml_modmanager.dll or alaydriem-mod-manager.dll) is loaded from "),
        "{line}"
    );
    assert!(line.ends_with("; this copy stays off"), "{line}");
}

#[test]
fn the_cpp_manager_module_keeps_this_copy_off() {
    let own = ManagerDll::load_copy();
    let cpp = ManagerDll::load_copy_as(DupGuard::CPP_MANAGER);

    let error = DupGuard::check(own.module).unwrap_err();

    let DupError::OtherCopy { path } = &error else { panic!("expected OtherCopy, got {error:?}") };
    assert_eq!(std::fs::canonicalize(path).unwrap(), std::fs::canonicalize(&cpp.path).unwrap());
}

#[test]
fn a_copy_loaded_under_another_name_stays_off() {
    // SAFETY: no input; the executable's own module
    let exe = unsafe { GetModuleHandleW(None) }.unwrap();
    assert!(matches!(DupGuard::check(exe), Err(DupError::NotFound)));
}

#[test]
fn enable_stays_off_when_a_manager_loaded_first_and_this_copy_came_after() {
    let first = ManagerDll::load_copy();
    let second = ManagerDll::load_copy();
    let started = Cell::new(false);

    let result = ManagerMod::enable_if_sole(Ok(second.module), || {
        started.set(true);
        Ok(())
    });

    let error = result.unwrap_err().to_string();
    assert!(error.contains("is loaded from "), "{error}");
    assert!(!started.get(), "enable must not start the host");
    drop(first);
}

#[test]
fn enable_starts_when_this_copy_is_the_one_mods_find() {
    let dll = ManagerDll::load_copy();
    let started = Cell::new(false);

    let result = ManagerMod::enable_if_sole(Ok(dll.module), || {
        started.set(true);
        Ok(())
    });

    assert!(result.is_ok());
    assert!(started.get());
}
