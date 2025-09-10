use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, ItemStruct, Meta};

#[proc_macro_attribute]
pub fn create_writer(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    // Parse the filename attribute
    let filename = if attr.is_empty() {
        return syn::Error::new_spanned(
            proc_macro2::TokenStream::from(attr),
            "A filename must be supplied when creating a commy writer!\nUse: #[create_writer(filename = \"my_mmap_filename.bin\")]"
        ).to_compile_error().into();
    } else {
        let meta: Meta = parse_macro_input!(attr as Meta);
        match meta {
            Meta::NameValue(nv) if nv.path.is_ident("filename") => {
                if let syn::Expr::Lit(lit) = &nv.value {
                    if let syn::Lit::Str(lit_str) = &lit.lit {
                        lit_str.value()
                    } else {
                        return syn::Error::new_spanned(
                            &nv.value,
                            "filename must be a string literal",
                        )
                        .to_compile_error()
                        .into();
                    }
                } else {
                    return syn::Error::new_spanned(&nv.value, "filename must be a string literal")
                        .to_compile_error()
                        .into();
                }
            }
            _ => {
                return syn::Error::new_spanned(
                    meta,
                    "Expected filename attribute like: filename = \"my_file.bin\"",
                )
                .to_compile_error()
                .into();
            }
        }
    };

    let input: ItemStruct = parse_macro_input!(item as ItemStruct);
    let struct_name = &input.ident;
    let struct_vis = &input.vis;

    // Process fields and create FieldHolder versions
    let mut field_definitions = TokenStream2::new();
    let mut field_initializers = TokenStream2::new();
    let mut field_accessors = TokenStream2::new();
    let mut field_setters = TokenStream2::new();

    for field in input.fields.iter() {
        let field_name = field.ident.as_ref().expect("All fields must be named");
        let field_type = &field.ty;
        let field_vis = &field.vis;

        field_definitions.extend(quote! {
            #field_vis #field_name: commy_common::FieldHolder<#field_type>,
        });

        field_initializers.extend(quote! {
            #field_name: commy_common::FieldHolder::new(Default::default(), writer_id),
        });

        // Create getter method
        let getter_name = quote::format_ident!("get_{}", field_name);
        field_accessors.extend(quote! {
            pub fn #getter_name(&self) -> &#field_type {
                self.#field_name.get()
            }
        });

        // Create setter method
        let setter_name = quote::format_ident!("set_{}", field_name);
        field_setters.extend(quote! {
            pub fn #setter_name(&mut self, value: #field_type) {
                self.#field_name.set(value, self.writer_id);
                // Invoke callback if registered
                let callback_key = format!("{}_{}", self.writer_id, std::any::type_name::<#field_type>());
                commy_common::invoke_callback(&callback_key, self.writer_id);
            }
        });
    }

    let expanded = quote! {
        #[repr(C)]
        #[derive(Debug)]
        #struct_vis struct #struct_name {
            writer_id: usize,
            #field_definitions
        }

        impl #struct_name {
            pub fn new() -> std::io::Result<commy_common::WriterStruct<'static, Self>> {
                let writer_struct = commy_common::WriterStruct::new(#filename)?;
                Ok(writer_struct)
            }

            pub fn writer_id(&self) -> usize {
                self.writer_id
            }

            #field_accessors
            #field_setters
        }

        impl commy_common::WithUniqueId for #struct_name {
            fn id_counter() -> &'static std::sync::atomic::AtomicUsize {
                static ID_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
                &ID_COUNTER
            }

            fn next_id() -> Result<usize, &'static str> {
                let counter = Self::id_counter();
                let id = counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if id == usize::MAX {
                    Err("Writer ID counter has overflowed")
                } else {
                    Ok(id)
                }
            }
        }

        impl Default for #struct_name {
            fn default() -> Self {
                let writer_id = Self::next_id().expect("Failed to generate writer ID");
                Self {
                    writer_id,
                    #field_initializers
                }
            }
        }
    };

    proc_macro::TokenStream::from(expanded)
}

/// A small helper proc-macro that generates a concrete rkyv writer
/// for the annotated struct. It emits an `impl` block with a
/// `pub fn write_into_buffer(&self, buf: &mut [u8]) -> Result<usize, Box<dyn std::error::Error>>`
/// method which calls `rkyv::to_bytes::<rkyv::rancor::BoxedError>(self)` and copies
/// the resulting bytes into the provided buffer.
#[proc_macro_attribute]
pub fn rkyv_writer(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    // Parse the input as a struct
    let input: ItemStruct = parse_macro_input!(item as ItemStruct);
    let struct_name = &input.ident;
    // Note: impl blocks cannot have visibility qualifiers like `pub impl`.
    // We'll generate a plain `impl` block and make individual methods public as needed.

    // Generate an impl block adding the write_into_buffer method.
    // The generated code assumes the consumer crate depends on `rkyv` and
    // that the type has the necessary `Archive`/`Serialize` derives.
    let expanded = quote! {
        #input

        #[allow(dead_code)]
        impl #struct_name {
            /// Serialize `self` directly into the provided `buf` without allocating an intermediate Vec.
            ///
            /// Returns the number of bytes written on success. On failure a CommyError is returned.
            ///
            /// Safety / Notes:
            /// - This method performs serialization using the `rkyv` crate and will attempt to write
            ///   the archived bytes into `buf`. If `buf` is too small the underlying writer will
            ///   return an error and this method will return Err(…).
            /// - We use fully-qualified paths so the generated code resolves in the consumer crate
            ///   that applies this proc-macro.
            pub fn write_into_buffer(&self, buf: &mut [u8]) -> Result<usize, ::commy::errors::CommyError> {
                // Wrap the mutable slice in rkyv's zero-allocation Buffer writer. This
                // implements the `Writer` and `Positional` traits rkyv needs.
                let writer = ::rkyv::ser::writer::Buffer::from(buf);

                // Use the high-level convenience helper which serializes into the provided
                // writer without allocating an intermediate Vec.
                let writer = match ::rkyv::api::high::to_bytes_in::<_, ::rkyv::rancor::Error>(self, writer) {
                    Ok(w) => w,
                    Err(e) => {
                        // Map the rkyv error into a crate-local BinarySerialization variant.
                        return Err(::commy::error::CommyError::BinarySerialization(format!(
                            "rkyv serialization failed: {}",
                            e
                        )));
                    }
                };

                // Return the number of bytes written into the buffer.
                Ok(::rkyv::ser::Positional::pos(&writer))
            }
        }
    };

    proc_macro::TokenStream::from(expanded)
}
