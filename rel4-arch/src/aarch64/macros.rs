/// Get value from the system register
#[macro_export]
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

/// Set the value of the system register
#[macro_export]
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
