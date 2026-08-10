//! Hardware variant selection (compile-time features).

#[cfg(all(
    feature = "variant-longfred-standard",
    feature = "variant-longfred-mini"
))]
compile_error!("enable only one hardware variant feature");
#[cfg(all(feature = "variant-longfred-standard", feature = "variant-markwtech"))]
compile_error!("enable only one hardware variant feature");
#[cfg(all(
    feature = "variant-longfred-standard",
    feature = "variant-heiko-wifred"
))]
compile_error!("enable only one hardware variant feature");
#[cfg(all(feature = "variant-longfred-mini", feature = "variant-markwtech"))]
compile_error!("enable only one hardware variant feature");
#[cfg(all(feature = "variant-longfred-mini", feature = "variant-heiko-wifred"))]
compile_error!("enable only one hardware variant feature");
#[cfg(all(feature = "variant-markwtech", feature = "variant-heiko-wifred"))]
compile_error!("enable only one hardware variant feature");

#[cfg(any(
    feature = "variant-longfred-standard",
    feature = "variant-longfred-mini"
))]
pub mod longfred_family;

#[cfg(feature = "variant-markwtech")]
pub mod markwtech;

#[cfg(feature = "variant-heiko-wifred")]
pub mod heiko_wifred;

use crate::board::descriptor::VariantDescriptor;

/// Active build variant descriptor.
pub fn active() -> &'static VariantDescriptor {
    #[cfg(feature = "variant-longfred-standard")]
    {
        return &longfred_family::STANDARD;
    }
    #[cfg(feature = "variant-longfred-mini")]
    {
        return &longfred_family::MINI;
    }
    #[cfg(feature = "variant-markwtech")]
    {
        return &markwtech::DESCRIPTOR;
    }
    #[cfg(feature = "variant-heiko-wifred")]
    {
        return &heiko_wifred::DESCRIPTOR;
    }
    #[cfg(not(any(
        feature = "variant-longfred-standard",
        feature = "variant-longfred-mini",
        feature = "variant-markwtech",
        feature = "variant-heiko-wifred"
    )))]
    {
        compile_error!("select a hardware variant feature");
    }
}

/// Alias for [`active`].
pub fn active_variant() -> &'static VariantDescriptor {
    active()
}

/// Control surface for the active LongFred-family variant.
#[cfg(any(
    feature = "variant-longfred-standard",
    feature = "variant-longfred-mini"
))]
pub fn surface() -> longfred_family::LongFredSurface {
    #[cfg(feature = "variant-longfred-mini")]
    {
        longfred_family::LongFredSurface::mini()
    }
    #[cfg(feature = "variant-longfred-standard")]
    {
        longfred_family::LongFredSurface::standard()
    }
}
