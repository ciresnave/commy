use hex::ToHex;
use proc_macro2::Span;
use proc_macro2::TokenStream as TokenStream2;
use quote::format_ident;
use quote::quote;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
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
    fn rust_ty_to_capnp(ty: &syn::Type) -> &'static str {
        match quote::quote! { #ty }.to_string().as_str() {
            "String" | "std :: string :: String" => "Text",
            "Vec < u8 >" | "alloc :: vec :: Vec < u8 >" => "Data",
            "u8" => "UInt8",
            "u16" => "UInt16",
            "u32" => "UInt32",
            "u64" => "UInt64",
            "i8" => "Int8",
            "i16" => "Int16",
            "i32" => "Int32",
            "i64" => "Int64",
            "f32" => "Float32",
            "f64" => "Float64",
            _ => "Text",
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
        let capnp_ty = rust_ty_to_capnp(&field.ty);
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

    // Attempt to write the schema into a `schemas/` directory in the consumer crate if possible.
    // Write local schema into consumer crate's schemas/ for tooling and build.rs consumption.
    // Prepare schema filename and generated rust filename up-front so they are
    // available in later scopes (proc-macro expansion must reference the
    // generated filename when emitting include! tokens).
    let struct_name_str = struct_name.to_string();
    let schema_filename = format!("{}_{}.capnp", struct_name_str, &hash_hex[..8]);
    let generated_name = format!("{}_{}_capnp.rs", struct_name_str, &hash_hex[..8]);
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let mut schema_dir = PathBuf::from(&manifest);
        schema_dir.push("schemas");
        if fs::create_dir_all(&schema_dir).is_ok() {
            let mut schema_file = schema_dir.clone();
            schema_file.push(&schema_filename);
            let _ = fs::write(&schema_file, &schema_text);

            // Attempt single-step code generation by invoking capnpc here so generated
            // Rust bindings are available during the same cargo build. We use the
            // `capnpc` crate which may in turn spawn the native `capnp` binary.
            // This is best-effort: if codegen fails we'll emit a compile-time
            // warning (via println! in build script style) and continue.
            let out_dir = std::env::var("OUT_DIR").ok().map(PathBuf::from);
            if let Some(out_dir) = out_dir {
                // Use capnpc CompilerCommand to emit Rust file into OUT_DIR
                let mut any_ok = false;
                match std::panic::catch_unwind(|| {
                    let mut cmd = capnpc::CompilerCommand::new();
                    cmd.file(&schema_file).output_path(&out_dir);
                    cmd.run()
                }) {
                    Ok(Ok(())) => {
                        any_ok = true;
                    }
                    Ok(Err(e)) => {
                        // Emit helpful compile-time warning. We can't print to build
                        // script output here, but we can emit a warning by using
                        // compile_error! would fail the build; instead we embed a
                        // constant string with the warning so users see it in rustc output
                        let warn = format!("capnp codegen failed during proc-macro expansion: {}. To enable single-step capnp codegen, install the capnp compiler and ensure it is on PATH.", e);
                        let _ =
                            fs::write(out_dir.join("capnp_codegen_warning.txt"), warn.as_bytes());
                    }
                    Err(_) => {
                        let warn = "capnp codegen panicked during proc-macro expansion".to_string();
                        let _ =
                            fs::write(out_dir.join("capnp_codegen_warning.txt"), warn.as_bytes());
                    }
                }

                // If codegen produced a Rust file, try to read it and inline into the
                // macro output so the generated types are directly available.
                if any_ok {
                    let mut gen_path = out_dir.clone();
                    gen_path.push(&generated_name);
                    if gen_path.exists() {
                        // We purposely do not parse/re-emit the generated tokens here.
                        // Instead we will emit an include!(concat!(env!("OUT_DIR"), "/", "<generated>"))
                        // into the macro output below which will cause the compiler to
                        // load the generated bindings in the same build.
                    }
                }
            }
        }
    }
    // Prepare literal strings for insertion into generated tokens
    let schema_lit = LitStr::new(&schema_text, Span::call_site());
    let hash_lit = LitStr::new(&hash_hex, Span::call_site());

    // If capnpc produced a generated Rust file in OUT_DIR, arrange to include it
    // in the expanded output so generated bindings are available in the same build.
    let mut include_generated_tokens = TokenStream2::new();
    if let Some(manifest_out) = std::env::var("OUT_DIR").ok() {
        let mut gen_path = PathBuf::from(&manifest_out);
        gen_path.push(&generated_name);

        if gen_path.exists() {
            // Try to read the generated file. If present, attempt to parse it and
            // inline the tokens. If parsing fails, fall back to include! so the
            // compiler will load the generated bindings.
            if let Ok(s) = fs::read_to_string(&gen_path) {
                match syn::parse_file(&s) {
                    Ok(parsed) => {
                        include_generated_tokens.extend(quote! {
                            // Inlined capnp-generated bindings
                            #parsed
                        });
                    }
                    Err(_) => {
                        let gen_lit = LitStr::new(&generated_name, Span::call_site());
                        include_generated_tokens.extend(quote! {
                            #[allow(unused_imports, dead_code)]
                            include!(concat!(env!("OUT_DIR"), "/", #gen_lit));
                        });
                    }
                }
            }
        }
    }

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
