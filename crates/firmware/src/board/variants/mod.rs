//! Hardware variant selection (compile-time features).

#[cfg(all(feature = "variant-longfred-v1", feature = "variant-markwtech"))]
compile_error!("enable only one hardware variant feature");

#[cfg(feature = "variant-longfred-v1")]
pub mod longfred_family;

#[cfg(feature = "variant-longfred-v1")]
pub mod longfred_v1;

#[cfg(feature = "variant-markwtech")]
pub mod markwtech;

use crate::board::descriptor::VariantDescriptor;

/// Active build variant descriptor.
pub fn active() -> &'static VariantDescriptor {
    #[cfg(feature = "variant-longfred-v1")]
    {
        return &longfred_family::V1;
    }
    #[cfg(feature = "variant-markwtech")]
    {
        return &markwtech::DESCRIPTOR;
    }
    #[cfg(not(any(feature = "variant-longfred-v1", feature = "variant-markwtech")))]
    {
        compile_error!("select a hardware variant feature");
    }
}

/// Alias for [`active`].
pub fn active_variant() -> &'static VariantDescriptor {
    active()
}
