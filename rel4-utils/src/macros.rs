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
