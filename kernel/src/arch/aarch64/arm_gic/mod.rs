pub mod gic_v2;
pub mod gic_v3;

use sel4_common::structures::irq_t;

cfg_if::cfg_if! {
    if #[cfg(feature = "ENABLE_SMP")] {
        use sel4_common::platform::{NUM_PPI_MINUS_ONE, NUM_PPI};
        use sel4_common::sel4_config::CONFIG_MAX_NUM_NODES;

        #[inline]
        pub(crate) fn irq_to_idx(irq: irq_t) -> usize {
            match irq.irq {
                0..=NUM_PPI_MINUS_ONE => irq.core * NUM_PPI + irq.irq,
                _ => (CONFIG_MAX_NUM_NODES - 1) * NUM_PPI + irq.irq,
            }
        }

    }
}