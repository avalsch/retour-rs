#![recursion_limit = "1024"]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(
  feature = "static-detour",
  feature(unboxed_closures, tuple_trait)
)]
#![cfg_attr(
  all(feature = "static-detour", test),
  feature(naked_functions)
)]

//! A cross-platform detour library written in Rust.
//!
//! ## Intro
//!
//! This library provides a thread-safe, inline detouring functionality by
//! disassembling and patching functions during runtime, using assembly opcodes
//! allocated within executable memory. It modifies the target functions and
//! replaces their prolog with an unconditional jump.
//!
//! Beyond the basic functionality this library handles several different edge
//! cases:
//!
//! - Relative branches.
//! - RIP relative operands.
//! - Detects NOP-padding.
//! - Relay for large offsets (>2GB).
//! - Supports hot patching.
//!
//! ## Detours
//!
//! Three different types of detours are provided:
//!
//! - [Static](./struct.StaticDetour.html): A static & type-safe interface.
//!   Thanks to its static nature it can accept a closure as its detour, but is
//!   required to be statically defined at compile time.
//!
//! - [Generic](./struct.GenericDetour.html): A type-safe interface — the same
//!   prototype is enforced for both the target and the detour. It is also
//!   enforced when invoking the original target.
//!
//! - [Raw](./struct.RawDetour.html): The underlying building block that the
//!   others types abstract upon. It has no type-safety and interacts with raw
//!   pointers. It should be avoided unless any types are references, or not
//!   known until runtime.
//! 
//! ## Supported Versions
//! This crate, with default features, will support the MSRV in `Cargo.toml` 
//! (currently 1.60.0). Certain features may require newer versions of the 
//! compiler, which will be documented here and in the docs. Any features 
//! that require the nightly compiler will always target the newest version.
//!
//! ## Features
//!
//! - **static-detour**: Required for static detours, due to usage
//!   of *unboxed_closures* and *tuple_trait*. The feature also enables a more
//!   extensive test suite. *Requires nightly compiler*
//! - **thiscall-abi**: Required for hooking functions that use the "thiscall" ABI. *Requires 1.73.0 or greater*
//! - **28-args**: Allows for detouring functions up to 28 arguments (default is 14)
//! - **42-args**: Allows for detouring functions up to 42 arguments
//!
//! ## Platforms
//!
//! - Both `x86` & `x86-64` are supported.
//!
//! ## Procedure
//!
//! To illustrate a detour on an x86 platform:
//!
//! ```c
//! 0 int return_five() {
//! 1     return 5;
//! 00400020 [b8 05 00 00 00] mov eax, 5
//! 00400025 [c3]             ret
//! 2 }
//! 3
//! 4 int detour_function() {
//! 5     return 10;
//! 00400040 [b8 0A 00 00 00] mov eax, 10
//! 00400045 [c3]             ret
//! 6 }
//! ```
//!
//! To detour `return_five` the library by default tries to replace five bytes
//! with a relative jump (the optimal scenario), which works in this case.
//! Executable memory will be allocated for the instruction and the function's
//! prolog will be replaced.
//!
//! ```c
//! 0 int return_five() {
//! 1     return detour_function();
//! 00400020 [e9 16 00 00 00] jmp 1b <detour_function>
//! 00400025 [c3]             ret
//! 2 }
//! 3
//! 4 int detour_function() {
//! 5     return 10;
//! 00400040 [b8 0a 00 00 00] mov eax, 10
//! 00400045 [c3]             ret
//! 6 }
//! ```
//!
//! Beyond what is shown here, a trampoline is also generated so the original
//! function can be called regardless whether the function is hooked or not.
//! 
//! For various injection methods, see the [README in the GitHub repo](https://github.com/Hpmason/retour-rs)

// Re-exports
pub use detours::*;
pub use error::{Error, Result};
pub use traits::{Function, HookableWith};

#[macro_use]
mod macros;

// Modules
mod alloc;
mod arch;
mod detours;
mod error;
mod pic;
mod traits;
mod util;

#[cfg(test)]
mod tests {
  use super::*;
  use crate::Result;
  use matches::assert_matches;

  #[test]
  fn detours_share_target() -> Result<()> {
    #[inline(never)]
    extern "C" fn add(x: i32, y: i32) -> i32 {
      unsafe { std::ptr::read_volatile(&x as *const i32) + y }
    }

    let hook1 = unsafe {
      extern "C" fn sub(x: i32, y: i32) -> i32 {
        x - y
      }
      GenericDetour::<extern "C" fn(i32, i32) -> i32>::new(add, sub)?
    };

    unsafe { hook1.enable()? };
    assert_eq!(add(5, 5), 0);

    let hook2 = unsafe {
      extern "C" fn div(x: i32, y: i32) -> i32 {
        x / y
      }
      GenericDetour::<extern "C" fn(i32, i32) -> i32>::new(add, div)?
    };

    unsafe { hook2.enable()? };

    // This will call the previous hook's detour
    assert_eq!(hook2.call(5, 5), 0);
    assert_eq!(add(10, 5), 2);
    Ok(())
  }

  #[test]
  fn same_detour_and_target() {
    #[inline(never)]
    extern "C" fn add(x: i32, y: i32) -> i32 {
      unsafe { std::ptr::read_volatile(&x as *const i32) + y }
    }

    let err = unsafe { RawDetour::new(add as *const (), add as *const ()).unwrap_err() };
    assert_matches!(err, Error::SameAddress);
  }
}
