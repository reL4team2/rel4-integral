mod boot;
mod c_traps;
mod consts;
mod exception;
pub(self) mod instruction;
mod pg;
mod platform;

#[cfg(feature = "BUILD_BINARY")]
mod fpu;

pub mod arm_gic;

pub use boot::try_init_kernel;
pub use c_traps::restore_user_context;
pub use exception::handleUnknownSyscall;
pub(crate) use pg::set_vm_root_for_flush;
pub use platform::init_freemem;

#[cfg(feature = "BUILD_BINARY")]
core::arch::global_asm!(include_str!("gen/head.S"));
#[cfg(feature = "BUILD_BINARY")]
core::arch::global_asm!(include_str!("gen/traps.S"));
