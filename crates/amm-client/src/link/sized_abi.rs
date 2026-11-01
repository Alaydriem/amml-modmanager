use amm_abi::types::AmmStatus;
use amm_core::services::abi::AbiPrefix;
use amm_core::traits::AbiSized;

/// Reads and writes a sized struct the manager shares, up to the size both sides know: the manager may be built
/// for an older or a newer minor version than this mod.
pub struct SizedAbi;

impl SizedAbi {
    /// The sized struct at `ptr`, read up to the size it declares and no further than this mod's own size. The
    /// fields the manager's struct does not have keep their blank value, and `size` holds the bytes read. Null or
    /// unaligned is `NullPointer`; a declared size below `T::BASE_SIZE` is `InvalidArgument`.
    ///
    /// # Safety
    ///
    /// `ptr` is null, or points at a struct that starts with its `u32` size and holds that many readable bytes for
    /// the call.
    /// The copy keeps the other side's bytes as they are, which is sound because every field of an `AbiSized`
    /// struct takes any byte pattern.
    pub unsafe fn read<T: AbiSized>(ptr: *const T) -> Result<T, AmmStatus> {
        if ptr.is_null() || !ptr.is_aligned() {
            return Err(AmmStatus::NullPointer);
        }
        // SAFETY: ptr is non-null and aligned for T, so for its leading u32, and the caller gives at least the size
        // field readable
        let declared = unsafe { ptr.cast::<u32>().read() };
        let len = AbiPrefix::len(declared, T::BASE_SIZE, size_of::<T>()).ok_or(AmmStatus::InvalidArgument)?;
        let mut value = T::blank();
        // SAFETY: the caller gives `declared` readable bytes at ptr and len is at most that; value holds
        // size_of::<T>() >= len bytes and is a local, so the two do not overlap
        unsafe { std::ptr::copy_nonoverlapping(ptr.cast::<u8>(), std::ptr::from_mut(&mut value).cast::<u8>(), len) };
        // SAFETY: the size field is a u32 at offset 0 of every AbiSized struct, and value is a live local
        unsafe { std::ptr::from_mut(&mut value).cast::<u32>().write(len as u32) };
        Ok(value)
    }

    /// Writes `value` to the sized struct at `ptr`, up to the size the manager declares in it and no further than
    /// this mod's own size, then puts the bytes written in its `size`. Null or unaligned is `NullPointer`; a declared
    /// size below `T::BASE_SIZE` is `InvalidArgument`, and nothing is written.
    ///
    /// # Safety
    ///
    /// `ptr` is null, or points at a struct that starts with its `u32` size and holds that many writable bytes for
    /// the call.
    /// The copy keeps the other side's bytes as they are, which is sound because every field of an `AbiSized`
    /// struct takes any byte pattern.
    pub unsafe fn write<T: AbiSized>(ptr: *mut T, value: &T) -> Result<(), AmmStatus> {
        if ptr.is_null() || !ptr.is_aligned() {
            return Err(AmmStatus::NullPointer);
        }
        // SAFETY: ptr is non-null and aligned for T, so for its leading u32, and the caller gives the size field
        let declared = unsafe { ptr.cast::<u32>().read() };
        let len = AbiPrefix::len(declared, T::BASE_SIZE, size_of::<T>()).ok_or(AmmStatus::InvalidArgument)?;
        // SAFETY: the caller gives `declared` writable bytes at ptr and len is at most that; value holds
        // size_of::<T>() >= len bytes and is borrowed, not the manager's struct
        unsafe { std::ptr::copy_nonoverlapping(std::ptr::from_ref(value).cast::<u8>(), ptr.cast::<u8>(), len) };
        // SAFETY: the size field is a u32 at offset 0 of every AbiSized struct, inside the bytes just written
        unsafe { ptr.cast::<u32>().write(len as u32) };
        Ok(())
    }
}
