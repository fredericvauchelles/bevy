use crate::plugin_deps_patch::input::PluginPatch;
use crate::{bevy_app_path, With};
use proc_macro2::*;
use quote::ToTokens;
use syn::punctuated::Punctuated;
use syn::token::*;
use syn::{Attribute, LitStr, Path, Token, TypePath};

pub fn execute(input: TokenStream) -> TokenStream {
    syn::parse2::<PluginPatch>(input)
        .and_then(|input| input.try_into())
        .map(|output: out::PluginPatchOutput| With(&output, &bevy_app_path()).to_token_stream())
        .unwrap_or_else(syn::Error::into_compile_error)
}

mod input {
    use super::*;
    use crate::ToTokensWith;
    use quote::quote;
    use syn::parse::{Parse, ParseStream};
    use syn::{braced, bracketed};

    pub struct PluginPatch {
        pub entries: Punctuated<PluginPatchEntry, Comma>,
    }
    impl Parse for PluginPatch {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            Ok(Self {
                entries: input.call(Punctuated::parse_terminated)?
            })
        }
    }

    pub struct PluginPatchEntry {
        pub id: PluginId,
        #[allow(unused)]
        pub semi: Token![:],
        #[allow(unused)]
        pub brace: Brace,
        pub deps: Punctuated<PluginDepsWithId, Comma>,
    }
    impl Parse for PluginPatchEntry {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let content;
            Ok(Self {
                id: input.parse()?,
                semi: input.parse()?,
                brace: braced!(content in input),
                deps: content.call(Punctuated::parse_terminated)?
            })
        }
    }

    pub struct PluginDepsWithId {
        pub ident: Ident,
        #[allow(unused)]
        pub semi: Token![:],
        pub deps: PluginDeps,
    }
    impl Parse for PluginDepsWithId {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            Ok(Self {
                ident: input.parse()?,
                semi: input.parse()?,
                deps: input.parse()?,
            })
        }
    }

    pub struct PluginDeps {
        #[allow(unused)]
        pub bracket: Bracket,
        pub deps: Punctuated<PluginDep, Comma>,
    }
    impl Parse for PluginDeps {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let content;
            Ok(Self {
                bracket: bracketed!(content in input),
                deps: content.call(Punctuated::parse_terminated)?,
            })
        }
    }

    #[derive(Clone)]
    pub enum PluginId {
        Literal(LitStr),
        Path(TypePath),
    }
    impl Parse for PluginId {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            if input.fork().parse::<LitStr>().is_ok() {
                Ok(Self::Literal(input.parse()?))
            } else {
                Ok(Self::Path(input.parse()?))
            }
        }
    }
    impl ToTokensWith<Path> for PluginId {
        fn to_tokens(&self, tokens: &mut TokenStream, bevy_app: &Path) {
            tokens.extend(match self {
                PluginId::Literal(v) => quote!(#bevy_app::prelude::PluginId::from(#v)),
                PluginId::Path(v) => quote!(#bevy_app::prelude::PluginId::of::<#v>()),
            });
        }
    }

    #[derive(Clone)]
    pub struct PluginDep {
        pub attrs: Vec<Attribute>,
        pub ty: PluginDepType,
        pub id: PluginId,
    }
    impl Parse for PluginDep {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            Ok(Self {
                attrs: input.call(Attribute::parse_outer)?,
                ty: input.parse()?,
                id: input.parse()?,
            })
        }
    }

    #[derive(Clone)]
    pub enum PluginDependencyOperation {
        #[allow(unused)]
        Add(Token![+]),
        #[allow(unused)]
        Remove(Token![-]),
    }
    impl PluginDependencyOperation {
        pub fn span(&self) -> Span {
            match self {
                PluginDependencyOperation::Add(t) => t.span,
                PluginDependencyOperation::Remove(t) => t.span,
            }
        }
    }
    impl Parse for PluginDependencyOperation {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            if input.peek(Token![+]) {
                Ok(Self::Add(input.parse()?))
            } else {
                Ok(Self::Remove(input.parse()?))
            }
        }
    }
    #[derive(Clone)]
    pub struct PluginDepType {
        pub op: PluginDependencyOperation,
        pub maybe_opt: Option<Token![?]>,
    }
    impl Parse for PluginDepType {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            Ok(Self {
                op: input.parse()?,
                maybe_opt: input.parse()?,
            })
        }
    }
}

mod out {
    use super::input::*;
    use super::*;
    use crate::{ToTokensWith, With};
    use quote::quote;

    pub struct PluginPatchOutput {
        entries: Vec<PluginPatchEntryOutput>,
    }

    impl ToTokensWith<Path> for PluginPatchOutput {
        fn to_tokens(&self, tokens: &mut TokenStream, bevy_app: &Path) {
            let _vec = quote!(#bevy_app::prelude::bevy_platform::prelude::vec);
            let entries = self.entries.iter().map(|entry| {
                let PluginPatchEntryOutput {
                    id,
                    dependencies
                } = entry;

                let id = With(id, bevy_app);

                let dependencies = dependencies.iter().map(|dep| {
                    let PluginPatchDependencyOutput {
                        op_ident,
                        kind_ident,
                        attrs,
                        id
                    } = dep;
                    let id = With(id, bevy_app);
                    quote! {
                        #(#attrs)*
                        #bevy_app::prelude::UpdatePluginDependency::#op_ident(
                            #bevy_app::prelude::PluginDependency::#kind_ident(
                                #id
                            )
                        )
                    }
                }).collect::<Vec<_>>();

                quote! {
                    (#id, #_vec![#(#dependencies),*].into_iter().collect())
                }
            }).collect::<Vec<_>>();

            tokens.extend(quote! {
                #bevy_app::prelude::PluginDependencyPatch::new(#_vec![#(#entries),*])
            });
        }
    }

    pub struct PluginPatchEntryOutput {
        id: PluginId,
        dependencies: Vec<PluginPatchDependencyOutput>,
    }

    pub struct PluginPatchDependencyOutput {
        op_ident: Ident,
        kind_ident: Ident,
        attrs: Vec<Attribute>,
        id: PluginId,
    }

    impl TryFrom<PluginPatch> for PluginPatchOutput {
        type Error = syn::Error;

        fn try_from(value: PluginPatch) -> Result<Self, Self::Error> {
            let entries = value.entries.into_iter()
                .map(|entry| {
                    Ok(PluginPatchEntryOutput {
                        id: entry.id,
                        dependencies: entry.deps.iter()
                            .flat_map(|dep| {
                                let suffix = if dep.ident == "build_before" {
                                    "Before"
                                } else if dep.ident == "build_after" {
                                    "After"
                                } else {
                                    return vec![Err(syn::Error::new(dep.ident.span(), format!("Invalid property: {}, expected either `build_before` or `build_after`", dep.ident)))];
                                };

                                dep.deps.deps.iter().map(|dep| {
                                    let op_ident = if matches!(dep.ty.op, PluginDependencyOperation::Add(_)) {
                                        Ident::new(if suffix == "Before" { "AddBefore" } else { "AddAfter" }, dep.ty.op.span())
                                    } else {
                                        Ident::new(if suffix == "Before" { "RemoveBefore" } else { "RemoveAfter" }, dep.ty.op.span())
                                    };

                                    let kind_ident = if let Some(t) = dep.ty.maybe_opt.as_ref() {
                                        Ident::new("Optional", t.span)
                                    } else {
                                        Ident::new("Required", dep.ty.op.span())
                                    };

                                    Ok(PluginPatchDependencyOutput {
                                        op_ident,
                                        kind_ident,
                                        attrs: dep.attrs.clone(),
                                        id: dep.id.clone(),
                                    })
                                })
                                    .collect()
                            })
                            .collect::<Result<Vec<_>, _>>()?,
                    })
                })
                .collect::<Result<Vec<_>, syn::Error>>()?;

            Ok(Self {
                entries
            })
        }
    }
}