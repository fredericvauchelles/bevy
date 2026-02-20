use crate::plugin_deps_patch::input::PluginDepsPatchInput;
use crate::{bevy_app_path, With};
use proc_macro2::*;
use quote::ToTokens;
use syn::punctuated::Punctuated;
use syn::token::*;
use syn::{Attribute, LitStr, Path, Token, TypePath};

pub fn execute(input: TokenStream) -> TokenStream {
    syn::parse2::<PluginDepsPatchInput>(input)
        .and_then(|input| input.try_into())
        .map(|output: out::Output| With(&output, &bevy_app_path()).to_token_stream())
        .unwrap_or_else(syn::Error::into_compile_error)
}

mod input {
    use super::*;
    use syn::bracketed;
    use syn::parse::{Parse, ParseStream};

    pub struct PluginDepsPatchInput {
        pub deps: Punctuated<PluginDepsWithId, Comma>,
    }

    impl Parse for PluginDepsPatchInput {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            Ok(Self {
                deps: input.call(Punctuated::parse_terminated)?,
            })
        }
    }

    pub struct PluginDepsWithId {
        pub ident: Ident,
        #[allow(dead_code)]
        pub eq: Token![:],
        pub deps: PluginDeps,
    }
    impl Parse for PluginDepsWithId {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            Ok(Self {
                ident: input.parse()?,
                eq: input.parse()?,
                deps: input.parse()?,
            })
        }
    }

    pub struct PluginDeps {
        #[allow(dead_code)]
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
    pub enum PluginDep {
        Literal(LiteralPluginDep),
        TypePath(TypePathPluginDep),
    }
    impl Parse for PluginDep {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            if input.fork().parse::<LiteralPluginDep>().is_ok() {
                Ok(Self::Literal(input.parse()?))
            } else {
                Ok(Self::TypePath(input.parse()?))
            }
        }
    }
    impl PluginDep {
        pub fn ty(&self) -> &PluginDepType {
            match self {
                PluginDep::Literal(v) => { &v.ty }
                PluginDep::TypePath(v) => { &v.ty }
            }
        }
    }

    #[derive(Clone)]
    pub struct LiteralPluginDep {
        pub ty: PluginDepType,
        pub attrs: Vec<Attribute>,
        pub literal: LitStr,
    }
    impl Parse for LiteralPluginDep {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            Ok(Self {
                ty: input.parse()?,
                attrs: input.call(Attribute::parse_outer)?,
                literal: input.parse()?,
            })
        }
    }

    #[derive(Clone)]
    pub struct TypePathPluginDep {
        pub ty: PluginDepType,
        pub attrs: Vec<Attribute>,
        pub path: TypePath,
    }
    impl Parse for TypePathPluginDep {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            Ok(Self {
                ty: input.parse()?,
                attrs: input.call(Attribute::parse_outer)?,
                path: input.parse()?,
            })
        }
    }
    #[derive(Clone)]
    pub enum PluginDepOp {
        #[allow(dead_code)]
        Add(Token![+]),
        #[allow(dead_code)]
        Remove(Token![-]),
    }
    impl Parse for PluginDepOp {
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
        pub op: PluginDepOp,
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

    pub struct Output {
        deps: Vec<PluginDepOut>,
    }

    impl TryFrom<PluginDepsPatchInput> for Output {
        type Error = syn::Error;

        fn try_from(value: PluginDepsPatchInput) -> Result<Self, Self::Error> {
            let deps = value.deps.into_iter()
                .flat_map(|dep| {
                    if dep.ident.eq("build_before") {
                        dep.deps.deps.iter()
                            .map(|dep| {
                                match &dep.ty().op {
                                    PluginDepOp::Add(_) => Ok(PluginDepOut::AddBefore(dep.clone())),
                                    PluginDepOp::Remove(_) => Ok(PluginDepOut::RemoveBefore(dep.clone())),
                                }
                            })
                            .collect::<Vec<_>>()
                    } else if dep.ident.eq("build_after") {
                        dep.deps.deps.iter()
                            .map(|dep| {
                                match &dep.ty().op {
                                    PluginDepOp::Add(_) => Ok(PluginDepOut::AddAfter(dep.clone())),
                                    PluginDepOp::Remove(_) => Ok(PluginDepOut::RemoveAfter(dep.clone())),
                                }
                            })
                            .collect()
                    } else {
                        vec![Err(syn::Error::new(dep.ident.span(), format!("Invalid property: {}, expected either `build_before` or `build_after`", dep.ident)))]
                    }
                }).collect::<Result<Vec<_>, _>>()?;

            Ok(Self {
                deps
            })
        }
    }
    pub enum PluginDepOut {
        AddBefore(PluginDep),
        AddAfter(PluginDep),
        RemoveBefore(PluginDep),
        RemoveAfter(PluginDep),
    }

    impl ToTokensWith<Path> for PluginDepOut {
        fn to_tokens(&self, tokens: &mut TokenStream, bevy_app: &Path) {
            let (op, dep) = match self {
                PluginDepOut::AddBefore(dep) => (quote!(AddBefore), dep),
                PluginDepOut::AddAfter(dep) => (quote!(AddAfter), dep),
                PluginDepOut::RemoveBefore(dep) => (quote!(RemoveBefore), dep),
                PluginDepOut::RemoveAfter(dep) => (quote!(RemoveAfter), dep),
            };
            let dep_ty = dep.ty().maybe_opt.is_some()
                .then(|| quote!(Optional)).unwrap_or_else(|| quote!(Required));
            let (dep, attr) = match dep {
                PluginDep::Literal(lit) => {
                    let s = &lit.literal;
                    (quote!(#bevy_app::prelude::PluginId::from(#s)), &lit.attrs)
                }
                PluginDep::TypePath(path) => {
                    let ty = &path.path;
                    (quote!(#bevy_app::prelude::PluginId::of::<#ty>()), &path.attrs)
                }
            };

            tokens.extend(quote! {
                #(#attr)*
                #bevy_app::prelude::UpdatePluginDependency::#op(#bevy_app::prelude::PluginDependency::#dep_ty(#dep))
            });
        }
    }

    impl ToTokensWith<Path> for Output {
        fn to_tokens(&self, tokens: &mut TokenStream, bevy_app: &Path) {
            let deps = self.deps.iter().map(|dep| With(dep, bevy_app)).collect::<Vec<_>>();
            tokens.extend(quote! {
                #bevy_app::prelude::bevy_platform::prelude::vec![
                    #(#deps),*
                ]
            })
        }
    }
}