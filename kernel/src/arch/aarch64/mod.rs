mod boot;
mod c_traps;
mod consts;
mod exception;
pub(self) mod instruction;
mod pg;
mod platform;

#[cfg(feature = "have_fpu")]
pub mod fpu;
pub mod arm_gic;
#[cfg(feature = "enable_smp")]
pub use arm_gic::ipi_send_target;

pub use boot::try_init_kernel;
pub use c_traps::{fastpath_restore, restore_user_context};
pub use exception::handle_unknown_syscall;
pub(crate) use pg::set_vm_root_for_flush;
pub use platform::init_freemem;

#[cfg(feature = "enable_smp")]
pub use boot::try_init_kernel_secondary_core;

#[cfg(feature = "enable_smp")]
pub mod smp;

#[cfg(feature = "enable_smp")]
pub use smp::*;
