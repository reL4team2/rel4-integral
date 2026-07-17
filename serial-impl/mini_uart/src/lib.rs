#![no_std]

//! Mini UART driver for BCM2711 (Raspberry Pi 4).
//!
//! The Mini UART is a 16550-like UART located inside the AUX peripheral block.
//!
//! AUX base: 0xfe215000
//! Register offsets from AUX base:
//!   AUX_ENABLES  (0x04): bit 0 = Mini UART enable
//!   AUX_MU_IO    (0x40): data register (write = TX, read = RX)
//!   AUX_MU_IER   (0x44): interrupt enable
//!   AUX_MU_IIR   (0x48): interrupt identify (bit 1 = TX empty, bit 2 = RX available)
//!   AUX_MU_LCR   (0x4c): line control (8-bit mode = 0x3)
//!   AUX_MU_MCR   (0x50): modem control
//!   AUX_MU_LSR   (0x54): line status (bit 5 = TX empty, bit 0 = RX ready)
//!   AUX_MU_MSR   (0x58): modem status
//!   AUX_MU_SCRATCH (0x5c)
//!   AUX_MU_CNTL  (0x60): control (bit 1 = TX enable, bit 0 = RX enable)
//!   AUX_MU_STAT  (0x64): extra status
//!   AUX_MU_BAUD  (0x68): baud rate divisor

use core::ptr::NonNull;
use serial_frame::SerialDriver;

pub struct MiniUart {
    base: NonNull<u32>,
}

unsafe impl Send for MiniUart {}
unsafe impl Sync for MiniUart {}

impl SerialDriver for MiniUart {
    /// `addr` points to the AUX base virtual address (mapped from phys 0xfe215000).
    fn new(addr: NonNull<usize>) -> Self {
        Self { base: addr.cast() }
    }

    fn init(&self) {
        unsafe {
            let enables = self.base.as_ptr().add(0x04 / 4);
            let cntl    = self.base.as_ptr().add(0x60 / 4);
            let lcr     = self.base.as_ptr().add(0x4c / 4);
            let baud    = self.base.as_ptr().add(0x68 / 4);
            let ier     = self.base.as_ptr().add(0x44 / 4);

            // Enable Mini UART in AUX_ENABLES
            core::ptr::write_volatile(enables, 1);

            // Disable TX/RX during configuration (CNTL = 0)
            core::ptr::write_volatile(cntl, 0);

            // 8-bit mode
            core::ptr::write_volatile(lcr, 0x3);

            // Disable interrupts
            core::ptr::write_volatile(ier, 0);

            // Baud rate divisor.  The UART clock on RPi4 is 500 MHz.
            // For 115200 baud: divisor = 500_000_000 / (8 * 115200) = 542.53 → 543
            core::ptr::write_volatile(baud, 543);

            // Enable TX and RX
            core::ptr::write_volatile(cntl, (1 << 1) | (1 << 0));
        }
    }

    fn putchar(&self, c: u8) {
        // Auto CR: insert '\r' before '\n' (matching platsupport SERIAL_AUTO_CR)
        if c == b'\n' {
            self.putchar(b'\r');
        }
        let lsr = unsafe { self.base.as_ptr().add(0x54 / 4) };
        let io  = unsafe { self.base.as_ptr().add(0x40 / 4) };
        // Wait while TX FIFO is *not* empty (LSR bit 5 == 0)
        while unsafe { core::ptr::read_volatile(lsr) } & (1 << 5) == 0 {
            core::hint::spin_loop();
        }
        unsafe { core::ptr::write_volatile(io, c as u32); }
    }

    fn getchar(&self) -> Option<u8> {
        let lsr = unsafe { self.base.as_ptr().add(0x54 / 4) };
        let io  = unsafe { self.base.as_ptr().add(0x40 / 4) };
        // LSR bit 0 = data ready
        if unsafe { core::ptr::read_volatile(lsr) } & 1 != 0 {
            Some(unsafe { core::ptr::read_volatile(io) } as u8)
        } else {
            None
        }
    }
}