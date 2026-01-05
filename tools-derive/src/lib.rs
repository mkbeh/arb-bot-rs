use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

/// An attribute macro to be applied to the `main` function in a binary crate.
/// This macro transforms the `main` function into an asynchronous entry point using Tokio's
/// runtime, initializes the application (e.g., logging and tracing setup), executes the
/// function body asynchronously, and handles any errors by logging them via `tracing`
/// and exiting the process with a non-zero code.
///
/// # Usage
///
/// Apply the macro directly to your `main` function. You can pass arguments to `tokio::main`
/// via the macro (e.g., `#[main(flavor = "multi_thread")]`).
#[proc_macro_attribute]
pub fn main(args: TokenStream, input: TokenStream) -> TokenStream {
    let func = parse_macro_input!(input as ItemFn);

    // Enforce that this macro is only usable on a function named 'main'.
    // This prevents misuse on other functions.
    if func.sig.ident != "main" {
        return syn::Error::new_spanned(
            &func.sig.ident,
            "this attribute can only be used on 'main'",
        )
        .to_compile_error()
        .into();
    }

    // Extract components from the parsed function for reconstruction.
    let fn_name = &func.sig.ident; // Function name ('main').
    let fn_body = &func.block; // Original function body (block).
    let fn_vis = &func.vis; // Visibility (e.g., pub, crate).
    let attrs = &func.attrs; // Existing attributes on the function.

    let fn_ret_type = match &func.sig.output {
        syn::ReturnType::Default => quote! { () },
        syn::ReturnType::Type(_, ty) => quote! { #ty },
    };

    // Convert macro arguments (e.g., flavor = "multi_thread") to tokens for passing to tokio::main.
    let args_tokens = proc_macro2::TokenStream::from(args);

    let expanded = quote! {
        #[tokio::main(#args_tokens)]
        #(#attrs)*
        #fn_vis async fn #fn_name() {
            #![allow(unused_must_use)]

            if let Err(e) = ::tools::setup_application(env!("CARGO_PKG_NAME")) {
                ::tracing::error!("Failed to initialize application: {:?}", e);
                ::std::process::exit(1);
            }

            let result: #fn_ret_type = async move #fn_body .await;

            if let Err(e) = result {
                ::tracing::error!("Application failed: {:?}", e);
                ::std::process::exit(1);
            }
        }
    };

    expanded.into()
}
