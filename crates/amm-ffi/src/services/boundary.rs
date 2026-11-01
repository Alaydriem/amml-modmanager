use amm_abi::types::AmmStatus;
use amm_core::services::PanicGuard;
use amm_core::services::abi::AbiPrefix;
use amm_core::traits::AbiSized;

/// The rules every entry point follows: no panic crosses into C or C++, and every pointer and length is checked
/// before it is read.
pub struct Boundary;

impl Boundary {
    /// Runs the body of an entry point. A panic becomes `AmmStatus::Panic` and one log line.
    pub fn guard<T>(f: impl FnOnce() -> Result<T, AmmStatus>) -> Result<T, AmmStatus> {
        PanicGuard::run(f, |payload| curia::error!("panic stopped at the manager boundary", { payload: payload }))
            .unwrap_or(Err(AmmStatus::Panic))
    }

    /// The status an entry point returns for a body result.
    pub fn status(result: Result<(), AmmStatus>) -> AmmStatus {
        match result {
            Ok(()) => AmmStatus::Ok,
            Err(status) => status,
        }
    }

    /// The bytes at `ptr`. A null pointer with a length of 0 is an empty slice; with any other length it is
    /// `NullPointer`.
    ///
    /// # Safety
    ///
    /// `ptr` is null or points at `len` readable bytes that stay valid and unchanged for `'a`.
    pub unsafe fn bytes<'a>(ptr: *const u8, len: usize) -> Result<&'a [u8], AmmStatus> {
        if len == 0 {
            return Ok(&[]);
        }
        if ptr.is_null() {
            return Err(AmmStatus::NullPointer);
        }
        // SAFETY: the caller guarantees len readable bytes at a non-null ptr for 'a
        Ok(unsafe { std::slice::from_raw_parts(ptr, len) })
    }

    /// The `len` values of `T` at `ptr`. A null pointer with a length of 0 is an empty slice; with any other length
    /// it is `NullPointer`, as is an unaligned pointer.
    ///
    /// # Safety
    ///
    /// `ptr` is null or points at `len` readable `T` that stay valid and unchanged for `'a`.
    pub unsafe fn values<'a, T>(ptr: *const T, len: usize) -> Result<&'a [T], AmmStatus> {
        if len == 0 {
            return Ok(&[]);
        }
        if ptr.is_null() || !ptr.is_aligned() {
            return Err(AmmStatus::NullPointer);
        }
        // SAFETY: the caller guarantees len readable T at a non-null ptr for 'a, and ptr is aligned, as checked
        Ok(unsafe { std::slice::from_raw_parts(ptr, len) })
    }

    /// The UTF-8 text at `ptr`, as `bytes` reads it. Text that is not UTF-8 is `InvalidArgument`.
    ///
    /// # Safety
    ///
    /// As `bytes`.
    pub unsafe fn text<'a>(ptr: *const u8, len: usize) -> Result<&'a str, AmmStatus> {
        // SAFETY: the caller upholds the contract of bytes
        let bytes = unsafe { Self::bytes(ptr, len) }?;
        std::str::from_utf8(bytes).map_err(|_| AmmStatus::InvalidArgument)
    }

    /// The text at `ptr` for a log line, where bad UTF-8 is replaced rather than refused. Null reads as empty.
    ///
    /// # Safety
    ///
    /// As `bytes`.
    pub unsafe fn lossy_text(ptr: *const u8, len: usize) -> String {
        // SAFETY: the caller upholds the contract of bytes
        let bytes = unsafe { Self::bytes(ptr, len) }.unwrap_or(&[]);
        String::from_utf8_lossy(bytes).into_owned()
    }

    /// The value at `ptr`, or `NullPointer`.
    ///
    /// # Safety
    ///
    /// `ptr` is null or points at a live, aligned `T` that stays valid for `'a`.
    pub unsafe fn input<'a, T>(ptr: *const T) -> Result<&'a T, AmmStatus> {
        // SAFETY: the caller guarantees a null or valid, aligned pointer for 'a
        unsafe { ptr.as_ref() }.ok_or(AmmStatus::NullPointer)
    }

    /// `ptr` when it can take a write of one `T`: not null and aligned. It does not write.
    pub fn writable<T>(ptr: *mut T) -> Result<*mut T, AmmStatus> {
        if ptr.is_null() || !ptr.is_aligned() {
            return Err(AmmStatus::NullPointer);
        }
        Ok(ptr)
    }

    /// The sized struct at `ptr`, read up to the size it declares and no further than this manager's own size. The
    /// fields the caller's struct does not have keep their blank value, and `size` holds the bytes read. Null or
    /// unaligned is `NullPointer`; a declared size below `T::BASE_SIZE` is `InvalidArgument`.
    ///
    /// # Safety
    ///
    /// `ptr` is null, or points at a struct that starts with its `u32` size and holds that many readable bytes for
    /// the call.
    /// The copy keeps the other side's bytes as they are, which is sound because every field of an `AbiSized`
    /// struct takes any byte pattern.
    pub unsafe fn sized_input<T: AbiSized>(ptr: *const T) -> Result<T, AmmStatus> {
        // SAFETY: the caller gives a null pointer or a struct whose size field is readable
        let len = unsafe { Self::sized_len(ptr) }?;
        let mut value = T::blank();
        // SAFETY: the caller gives `declared` readable bytes at ptr and len is at most that; value holds
        // size_of::<T>() >= len bytes and is a local, so the two do not overlap
        unsafe { std::ptr::copy_nonoverlapping(ptr.cast::<u8>(), std::ptr::from_mut(&mut value).cast::<u8>(), len) };
        // SAFETY: the size field is a u32 at offset 0 of every AbiSized struct, and value is a live local
        unsafe { std::ptr::from_mut(&mut value).cast::<u32>().write(len as u32) };
        Ok(value)
    }

    /// Writes `value` to the sized struct at `ptr`, up to the size the caller declares in it and no further than
    /// this manager's own size, then puts the bytes written in its `size`. Null or unaligned is `NullPointer`; a
    /// declared size below `T::BASE_SIZE` is `InvalidArgument`, and nothing is written.
    ///
    /// # Safety
    ///
    /// `ptr` is null, or points at a struct that starts with its `u32` size and holds that many writable bytes for
    /// the call.
    /// The copy keeps the other side's bytes as they are, which is sound because every field of an `AbiSized`
    /// struct takes any byte pattern.
    pub unsafe fn sized_output<T: AbiSized>(ptr: *mut T, value: &T) -> Result<(), AmmStatus> {
        // SAFETY: the caller gives a null pointer or a struct whose size field is readable
        let len = unsafe { Self::sized_len(ptr.cast_const()) }?;
        // SAFETY: the caller gives `declared` writable bytes at ptr and len is at most that; value holds
        // size_of::<T>() >= len bytes and is borrowed, not the caller's struct
        unsafe { std::ptr::copy_nonoverlapping(std::ptr::from_ref(value).cast::<u8>(), ptr.cast::<u8>(), len) };
        // SAFETY: the size field is a u32 at offset 0 of every AbiSized struct, inside the bytes just written
        unsafe { ptr.cast::<u32>().write(len as u32) };
        Ok(())
    }

    /// The bytes `sized_input` or `sized_output` would read or write at `ptr`, found from the size it declares,
    /// without reading or writing anything else. An entry point that must not change state for a struct it will
    /// refuse checks this first. Null or unaligned is `NullPointer`; a declared size below `T::BASE_SIZE` is
    /// `InvalidArgument`.
    ///
    /// # Safety
    ///
    /// `ptr` is null, or points at a struct that starts with its `u32` size, readable for the call.
    pub unsafe fn sized_len<T: AbiSized>(ptr: *const T) -> Result<usize, AmmStatus> {
        if ptr.is_null() || !ptr.is_aligned() {
            return Err(AmmStatus::NullPointer);
        }
        // SAFETY: ptr is non-null and aligned for T, so for its leading u32, and the caller gives the size field
        let declared = unsafe { ptr.cast::<u32>().read() };
        AbiPrefix::len(declared, T::BASE_SIZE, size_of::<T>()).ok_or(AmmStatus::InvalidArgument)
    }
}
