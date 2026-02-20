mod plugin_deps_patch;

use bevy_macro_utils::BevyManifest;
use proc_macro::TokenStream;
use quote::ToTokens;

#[proc_macro]
pub fn plugin_deps_patch(input: TokenStream) -> TokenStream {
    plugin_deps_patch::execute(input.into()).into()
}

fn bevy_app_path() -> syn::Path {
    BevyManifest::shared(|manifest| manifest.get_path("bevy_app"))
}

trait ToTokensWith<T> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream, with: &T);
}

struct With<U, T>(U, T);
impl<U: ToTokensWith<T>, T> ToTokens for With<&U, &T> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.0.to_tokens(tokens, self.1)
    }
}
