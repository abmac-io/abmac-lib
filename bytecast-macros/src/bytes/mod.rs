//! Derive macros for bytecast.

mod from_bytes;
mod to_bytes;

pub use from_bytes::derive_from_bytes;
pub use to_bytes::derive_to_bytes;

/// Extract the discriminant type from `#[repr(uN)]` on an enum.
/// Returns `None` if no repr or a non-integer repr is used (defaults to u8).
pub fn repr_int_type(attrs: &[syn::Attribute]) -> Option<syn::Ident> {
    for attr in attrs {
        if !attr.path().is_ident("repr") {
            continue;
        }
        let mut found = None;
        let _ = attr.parse_nested_meta(|meta| {
            let ident = meta.path.get_ident().map(|id| id.to_string());
            if let Some("u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32" | "i64") =
                ident.as_deref()
            {
                found = Some(meta.path.get_ident().unwrap().clone());
            }
            Ok(())
        });
        if found.is_some() {
            return found;
        }
    }
    None
}

/// Return the max number of variants a discriminant type can hold.
pub fn disc_capacity(disc_type: &str) -> usize {
    match disc_type {
        "u8" | "i8" => 256,
        "u16" | "i16" => 65536,
        _ => usize::MAX,
    }
}

/// Check if a field has `#[bytecast(name)]` for the given attribute name.
fn has_bytecast_attr(field: &syn::Field, name: &str) -> bool {
    field.attrs.iter().any(|attr| {
        if !attr.path().is_ident("bytecast") {
            return false;
        }
        let mut found = false;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident(name) {
                found = true;
            }
            Ok(())
        });
        found
    })
}

pub fn has_skip_attr(field: &syn::Field) -> bool {
    has_bytecast_attr(field, "skip") || is_phantom_data(&field.ty)
}

/// Check if a type is `PhantomData` (with any generic args).
fn is_phantom_data(ty: &syn::Type) -> bool {
    let syn::Type::Path(type_path) = ty else {
        return false;
    };
    type_path
        .path
        .segments
        .last()
        .is_some_and(|seg| seg.ident == "PhantomData")
}

pub fn has_boxed_attr(field: &syn::Field) -> bool {
    has_bytecast_attr(field, "boxed")
}

/// Extract the inner type `T` from `Box<T>`.
pub fn extract_box_inner(ty: &syn::Type) -> Option<&syn::Type> {
    let syn::Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    if segment.ident != "Box" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    match args.args.first() {
        Some(syn::GenericArgument::Type(inner)) => Some(inner),
        _ => None,
    }
}

/// Resolve the serializable type for a field, accounting for `#[bytecast(boxed)]`.
pub fn serializable_type(field: &syn::Field) -> &syn::Type {
    if has_boxed_attr(field) {
        extract_box_inner(&field.ty).expect("#[bytecast(boxed)] requires field type to be Box<T>")
    } else {
        &field.ty
    }
}
