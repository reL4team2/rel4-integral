/// Get value from the system register
macro_rules! mrs {
    ($reg: literal) => {
        {
            let value: usize;
            unsafe {
                core::arch::asm!(concat!("mrs {0}, ", $reg), out(reg) value);
            }
            value
        }
    };
}

#[allow(unused)]
/// Set the value of the system register
macro_rules! msr {
    ($reg: literal, $v: literal) => {
        unsafe {
            core::arch::asm!(concat!("msr ", $reg, ", {0}"), const $v);
        }
    };
    ($reg: literal, $v: ident) => {
        unsafe {
            core::arch::asm!(concat!("msr ", $reg, ", {0}"), in(reg) $v);
        }
    };
}

/// Get the value of the FAR register.
#[inline]
pub fn get_far() -> usize {
    mrs!("far_el1")
}

#[inline]
pub fn get_esr() -> usize {
    mrs!("esr_el1")
}
