use proc_macro::TokenStream;
use quote::quote;
use syn::visit_mut::{self, VisitMut};
use syn::{
    FnArg, GenericArgument, ImplItem, ItemImpl, PathArguments, ReturnType, Type, parse_macro_input,
};

struct QualifySelf<'a> {
    self_ty: &'a Type,
}

impl VisitMut for QualifySelf<'_> {
    fn visit_type_mut(&mut self, ty: &mut Type) {
        if let Type::Path(path) = ty
            && path.qself.is_none()
            && path.path.segments.len() == 2
            && path.path.segments[0].ident == "Self"
        {
            let assoc = path.path.segments[1].ident.clone();
            let self_ty = self.self_ty;
            *ty = syn::parse_quote!(<#self_ty as ::ast::AstTransform>::#assoc);
            return;
        }
        visit_mut::visit_type_mut(self, ty);
    }
}

fn ast_node_inner(ty: &Type) -> syn::Result<Type> {
    let Type::Path(path) = ty else {
        return Err(syn::Error::new_spanned(ty, "expected AstNode<T, A>"));
    };
    let segment = path
        .path
        .segments
        .last()
        .ok_or_else(|| syn::Error::new_spanned(ty, "expected AstNode<T, A>"))?;
    if segment.ident != "AstNode" {
        return Err(syn::Error::new_spanned(ty, "expected AstNode<T, A>"));
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Err(syn::Error::new_spanned(ty, "expected AstNode<T, A>"));
    };
    match args.args.first() {
        Some(GenericArgument::Type(inner)) => Ok(inner.clone()),
        _ => Err(syn::Error::new_spanned(ty, "expected AstNode<T, A>")),
    }
}

fn result_node_inner(output: &ReturnType) -> syn::Result<Type> {
    let ReturnType::Type(_, ty) = output else {
        return Err(syn::Error::new_spanned(
            output,
            "transform methods must return Results<AstNode<...>, ...>",
        ));
    };
    let Type::Path(path) = &**ty else {
        return Err(syn::Error::new_spanned(
            ty,
            "expected Results<AstNode<...>, ...>",
        ));
    };
    let segment = path.path.segments.last().unwrap();
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Err(syn::Error::new_spanned(
            ty,
            "expected Results<AstNode<...>, ...>",
        ));
    };
    let Some(GenericArgument::Type(node_ty)) = args.args.first() else {
        return Err(syn::Error::new_spanned(
            ty,
            "expected Results<AstNode<...>, ...>",
        ));
    };
    ast_node_inner(node_ty)
}

#[proc_macro_attribute]
pub fn transformer(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemImpl);
    let Some((_, trait_path, _)) = &input.trait_ else {
        return syn::Error::new_spanned(
            &input,
            "#[transformer] requires an impl AstTransform block",
        )
        .into_compile_error()
        .into();
    };
    if trait_path
        .segments
        .last()
        .is_none_or(|s| s.ident != "AstTransform")
    {
        return syn::Error::new_spanned(
            trait_path,
            "#[transformer] requires an impl AstTransform block",
        )
        .into_compile_error()
        .into();
    }

    let self_ty = &input.self_ty;
    let generics = &input.generics;
    let (impl_generics, _, where_clause) = generics.split_for_impl();
    let mut base_items = Vec::new();
    let mut transforms = Vec::new();

    for item in &input.items {
        match item {
            ImplItem::Type(_) | ImplItem::Const(_) => base_items.push(quote!(#item)),
            ImplItem::Fn(method) => {
                let Some(FnArg::Typed(arg)) = method.sig.inputs.iter().nth(1) else {
                    return syn::Error::new_spanned(
                        &method.sig,
                        "transform methods need a node argument",
                    )
                    .into_compile_error()
                    .into();
                };
                let mut input_ty = match ast_node_inner(&arg.ty) {
                    Ok(ty) => ty,
                    Err(err) => return err.into_compile_error().into(),
                };
                let mut output_ty = match result_node_inner(&method.sig.output) {
                    Ok(ty) => ty,
                    Err(err) => return err.into_compile_error().into(),
                };
                QualifySelf { self_ty }.visit_type_mut(&mut input_ty);
                QualifySelf { self_ty }.visit_type_mut(&mut output_ty);
                let attrs = &method.attrs;
                let block = &method.block;
                let node_pat = &arg.pat;
                transforms.push(quote! {
                    #(#attrs)*
                    impl #impl_generics ::ast::Transform<#input_ty, #output_ty> for #self_ty #where_clause {
                        fn transform(
                            &mut self,
                            #node_pat: ::ast::AstNode<#input_ty, Self::From>,
                        ) -> ::ast::Results<::ast::AstNode<#output_ty, Self::To>, Self::Error>
                        #block
                    }
                });
            }
            ImplItem::Macro(item_macro) => {
                let macro_name = item_macro
                    .mac
                    .path
                    .segments
                    .last()
                    .unwrap()
                    .ident
                    .to_string();
                let args = match syn::parse2::<IdentityArgs>(item_macro.mac.tokens.clone()) {
                    Ok(args) => args,
                    Err(err) => return err.into_compile_error().into(),
                };
                let ty = args.ty;
                let from_ty: Type = if macro_name == "identity_generic" {
                    syn::parse_quote!(#ty<<#self_ty as ::ast::AstTransform>::From>)
                } else if macro_name == "identity_leaf" {
                    ty.clone()
                } else {
                    return syn::Error::new_spanned(item_macro, "unsupported macro in transformer")
                        .into_compile_error()
                        .into();
                };
                let to_ty: Type = if macro_name == "identity_generic" {
                    syn::parse_quote!(#ty<<#self_ty as ::ast::AstTransform>::To>)
                } else {
                    ty
                };
                transforms.push(quote! {
                    impl #impl_generics ::ast::Transform<#from_ty, #to_ty> for #self_ty #where_clause {
                        fn transform(
                            &mut self,
                            node: ::ast::AstNode<#from_ty, Self::From>,
                        ) -> ::ast::Results<::ast::AstNode<#to_ty, Self::To>, Self::Error> {
                            ::ast::Results::ok(::ast::AstNode { inner: node.inner, meta: node.meta })
                        }
                    }
                });
            }
            other => {
                return syn::Error::new_spanned(other, "unsupported item in transformer")
                    .into_compile_error()
                    .into();
            }
        }
    }

    quote! {
        impl #impl_generics #trait_path for #self_ty #where_clause {
            #(#base_items)*
        }
        #(#transforms)*
    }
    .into()
}

struct IdentityArgs {
    _name: syn::Ident,
    ty: Type,
}

impl syn::parse::Parse for IdentityArgs {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let name = input.parse()?;
        input.parse::<syn::Token![,]>()?;
        let ty = input.parse()?;
        Ok(Self { _name: name, ty })
    }
}
