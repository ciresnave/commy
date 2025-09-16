use hex::ToHex;
use proc_macro2::Span;
use proc_macro2::TokenStream as TokenStream2;
use quote::format_ident;
use quote::quote;
use sha2::{Digest, Sha256};
// (no filesystem writes from the macro; build.rs handles codegen)
use syn::{parse_macro_input, ItemStruct, LitStr, Meta};

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

    // Build a deterministic Cap'n Proto schema for this struct.
    // Map primitive Rust types to Cap'n Proto types (best-effort mapping for common types).
    fn rust_ty_to_capnp(ty: &syn::Type) -> Result<&'static str, String> {
        use syn::{GenericArgument, PathArguments, Type, TypePath};

        match ty {
            Type::Path(TypePath { path, .. }) => {
                if let Some(seg) = path.segments.last() {
                    let ident = seg.ident.to_string();
                    match ident.as_str() {
                        "String" => Ok("Text"),
                        "u8" => Ok("UInt8"),
                        "u16" => Ok("UInt16"),
                        "u32" => Ok("UInt32"),
                        "u64" => Ok("UInt64"),
                        "i8" => Ok("Int8"),
                        "i16" => Ok("Int16"),
                        "i32" => Ok("Int32"),
                        "i64" => Ok("Int64"),
                        "f32" => Ok("Float32"),
                        "f64" => Ok("Float64"),
                        "Vec" => {
                            // Check for Vec<u8>
                            match &seg.arguments {
                                PathArguments::AngleBracketed(args) => {
                                    if let Some(GenericArgument::Type(Type::Path(TypePath {
                                        path: inner_path,
                                        ..
                                    }))) = args.args.first()
                                    {
                                        if let Some(inner_seg) = inner_path.segments.last() {
                                            if inner_seg.ident == "u8" {
                                                return Ok("Data");
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                            Ok("Text")
                        }
                        _ => {
                            // Fallback for fully-qualified paths like std::string::String
                            let full = quote::quote! { #ty }.to_string();
                            if full.contains("std :: string :: String")
                                || full.contains("std::string::String")
                            {
                                Ok("Text")
                            } else if full.contains("alloc :: vec :: Vec") {
                                Ok("Data")
                            } else {
                                Err(format!("Unmapped Rust type '{}' encountered in Cap'n Proto schema generation. Please add a mapping for this type.", full))
                            }
                        }
                    }
                } else {
                    Err(format!(
                        "Unable to determine path segments for type: {}",
                        quote::quote! { #ty }.to_string()
                    ))
                }
            }
            _ => Err(format!(
                "Unsupported type form for Cap'n Proto mapping: {}",
                quote::quote! { #ty }.to_string()
            )),
        }
    }

    // Construct the schema text deterministically: fields in declared order, simple formatting.
    let mut schema_lines = Vec::new();
    schema_lines.push(format!("@0x{};", "{placeholder}")); // placeholder for hash-based id
    schema_lines.push(format!("struct {} {{", struct_name));
    let mut idx = 0usize;
    for field in input.fields.iter() {
        let field_name = field
            .ident
            .as_ref()
            .expect("All fields must be named")
            .to_string();
        // Map the Rust type to a Cap'n Proto type, returning a compile-time error
        // if an unmapped or unsupported type is encountered.
        let capnp_ty = match rust_ty_to_capnp(&field.ty) {
            Ok(t) => t.to_string(),
            Err(e) => {
                return syn::Error::new_spanned(
                    &field.ty,
                    format!(
                        "Error generating Cap'n Proto schema for field '{}': {}",
                        field_name, e
                    ),
                )
                .to_compile_error()
                .into();
            }
        };
        schema_lines.push(format!("  {} @{} :{};", field_name, idx, capnp_ty));
        idx += 1;
    }
    schema_lines.push("}".to_string());

    let schema_text = schema_lines.join("\n");

    // Compute SHA-256 hash of the schema text
    let mut hasher = Sha256::new();
    hasher.update(schema_text.as_bytes());
    let hash = hasher.finalize();
    let hash_hex = hash.encode_hex::<String>();

    // Replace placeholder id with the first 16 hex chars to form @0xID
    let short_id = &hash_hex[..16];
    let schema_text = schema_text.replacen("{placeholder}", short_id, 1);

    // Prepare schema and generated filenames (used for include! below).
    let struct_name_str = struct_name.to_string();
    let generated_basename = format!("{}_{}", struct_name_str, &hash_hex[..8]);
    // Prepare literal strings for insertion into generated tokens
    let schema_lit = LitStr::new(&schema_text, Span::call_site());
    let hash_lit = LitStr::new(&hash_hex, Span::call_site());

    // Emit a safe include for generated bindings. We do not perform any file
    // system writes or run capnpc from inside the proc-macro. The project's
    // top-level `build.rs` is responsible for running capnpc and placing
    // generated bindings into OUT_DIR. If the `capnproto` feature is enabled
    // but the build script did not produce bindings, we emit a helpful
    // compile-time error so the developer knows how to proceed.
    let gen_basename_lit = LitStr::new(&generated_basename, Span::call_site());
    let include_generated_tokens = quote! {
        // Include generated bindings only when the build script produced them
        // and the `capnp_generated` cfg was set.
        #[cfg(all(feature = "capnproto", capnp_generated))]
        {
            #[allow(unused_imports, dead_code)]
            include!(concat!(env!("OUT_DIR"), "/", #gen_basename_lit, "_capnp.rs"));
        }

        // If the user enabled `capnproto` but codegen did not run successfully,
        // provide a clear compile-time error with actionable next steps.
        #[cfg(all(feature = "capnproto", not(capnp_generated)))]
        compile_error!("capnp codegen bindings not available; install the `capnp` compiler (https://capnproto.org/install.html), enable the `capnproto` feature and re-run the build (try `cargo clean` if issues persist)");
    };

    // Make unique constant names per-struct to avoid duplicates when macro is applied to
    // multiple structs in the same crate. e.g., SCHEMA_TEXT_MyStruct
    let name_str = struct_name.to_string();
    let schema_ident = format_ident!("SCHEMA_TEXT_{}", name_str);
    let hash_ident = format_ident!("SCHEMA_HASH_{}", name_str);

    let expanded = quote! {
        #include_generated_tokens
        // Export the generated schema text and hash so build.rs or other tooling can find it.
        pub const #schema_ident: &str = #schema_lit;
        pub const #hash_ident: &str = #hash_lit;
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
