#[macro_export]
macro_rules! bit {
    ($e:expr) => {
        (1 << ($e))
    };
}

#[macro_export]
macro_rules! bits {
    ($($e:expr),*) => {
        $( (1 << ($e)) )|*
    };
}

#[macro_export]
macro_rules! mask_bits {
    ($e:expr) => {
        ((1usize << $e) - 1usize)
    }
}

#[macro_export]
/// Check if the given number is aligned to the given number of bits.
macro_rules! is_aligned {
    ($n:expr,$b:expr) => {
        ($n & mask_bits!($b) == 0)
    };
}

#[macro_export]
/// Calculate the floor of the given number.
macro_rules! round_down {
    ($n:expr,$b:expr) => {
        ((($n) >> ($b)) << ($b))
    };
}

#[macro_export]
/// Calculate the ceil of the given number.
macro_rules! round_up {
    ($n:expr,$b:expr) => {
        ((((($n) - 1usize) >> ($b)) + 1usize) << ($b))
    };
}

/// Implemente generic function for given Ident
///
/// ## Example
///
/// ```rust
/// // Impl new function to Struct A and Struct B
/// struct A(usize);
/// struct B(usize);
/// impl_multi(A, B {
///     pub const fn new(val: usize) -> Self {
///         Self(val)
///     }
/// })
/// ```
#[macro_export]
macro_rules! impl_multi {
    ($($t:ident),* {$($block:item)*}) => {
        macro_rules! methods {
            () => {
                $($block)*
            };
        }
        $(
            impl $t {
                methods!();
            }
        )*
    }
}
