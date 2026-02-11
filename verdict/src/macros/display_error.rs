/// Define an error enum with `Display` and `Error` impls.
///
/// Unlike [`error!`], this macro does not generate an `Actionable` impl,
/// making it suitable for crates that depend on verdict only optionally.
///
/// Each variant requires a `#[display("...")]` string with optional field
/// interpolation.
///
/// # Example
///
/// ```
/// use verdict::display_error;
///
/// display_error! {
///     #[derive(Clone, PartialEq, Eq)]
///     pub enum StorageError {
///         #[display("I/O error")]
///         Io,
///
///         #[display("not found")]
///         NotFound,
///
///         #[display("checksum mismatch: expected {expected:#x}, got {actual:#x}")]
///         ChecksumMismatch { expected: u32, actual: u32 },
///     }
/// }
///
/// let err = StorageError::Io;
/// assert_eq!(err.to_string(), "I/O error");
///
/// let err = StorageError::ChecksumMismatch { expected: 0xAB, actual: 0xCD };
/// assert_eq!(err.to_string(), "checksum mismatch: expected 0xab, got 0xcd");
/// ```
#[macro_export]
macro_rules! display_error {
    // Entry point — parse the enum, then dispatch to TT-muncher for Display
    (
        $(#[$attr:meta])*
        $vis:vis enum $name:ident {
            $(
                #[display($fmt:literal)]
                $variant:ident $({ $($field:ident : $fty:ty),* $(,)? })?
            ),* $(,)?
        }
    ) => {
        #[derive(Debug)]
        $(#[$attr])*
        #[must_use]
        $vis enum $name {
            $(
                $variant $({ $($field : $fty),* })?,
            )*
        }

        impl ::core::error::Error for $name {}

        // Kick off Display TT-muncher with empty accumulator
        $crate::display_error!(@build_display $name [] $(
            [$variant $({ $($field),* })? => $fmt]
        )*);
    };

    // Display TT-muncher

    // Base case: no more variants, emit the impl
    (@build_display $name:ident [ $($arms:tt)* ]) => {
        $crate::display_error!(@emit_display $name $($arms)*);
    };

    // Struct variant — bind fields for interpolation
    (@build_display $name:ident [ $($arms:tt)* ]
        [$variant:ident { $($field:ident),* } => $fmt:literal]
        $($rest:tt)*
    ) => {
        $crate::display_error!(@build_display $name [
            $($arms)*
            { $name :: $variant { $($field),* } => $fmt }
        ] $($rest)*);
    };

    // Unit variant — no fields
    (@build_display $name:ident [ $($arms:tt)* ]
        [$variant:ident => $fmt:literal]
        $($rest:tt)*
    ) => {
        $crate::display_error!(@build_display $name [
            $($arms)*
            { $name :: $variant => $fmt }
        ] $($rest)*);
    };

    // Emit Display impl — `f` is introduced here at a single hygiene level
    (@emit_display $name:ident $( { $pat:pat => $fmt:literal } )* ) => {
        impl ::core::fmt::Display for $name {
            #[allow(unused_variables)]
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                match self {
                    $(
                        $pat => ::core::write!(f, $fmt),
                    )*
                }
            }
        }
    };
}
