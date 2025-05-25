use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};
use std::collections::BTreeSet;
use syn::{
    DeriveInput, LifetimeParam, LitStr, Meta, Path, Token, parse::Parser, parse_macro_input,
    punctuated::Punctuated,
};

// 用于比较类型的辅助结构
#[derive(Ord, PartialOrd, Eq, PartialEq)]
struct TypeIdentifier(String);

fn get_type_identifier(ty: &syn::Type) -> TypeIdentifier {
    TypeIdentifier(quote!(#ty).to_string())
}
/// 处理并增强 `#[template]` 属性，添加必要的默认值
fn process_template_attr(input: &DeriveInput) -> Punctuated<Meta, Token![,]> {
    let mut args = Punctuated::<Meta, Token![,]>::new();
    for attr in &input.attrs {
        if !attr.path().is_ident("template") {
            continue;
        }
        // 处理template属性
        let mut has_askama = false;
        let mut has_source = false;
        let mut has_ext = false;

        let nested = match attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated) {
            Ok(n) => n,
            Err(_) => continue,
        };
        for meta in &nested {
            if meta.path().is_ident("source") {
                has_source = true;
            }
            if meta.path().is_ident("ext") {
                has_ext = true;
            }
            if meta.path().is_ident("askama") {
                has_askama = true;
            }
            args.push(meta.clone());
        }

        // 设置默认值

        if !has_askama {
            let askama_meta = Meta::NameValue(syn::MetaNameValue {
                path: syn::Path::from(syn::Ident::new("askama", Span::call_site())),
                eq_token: <syn::Token![=]>::default(),
                value: syn::Expr::Path(syn::ExprPath {
                    attrs: Vec::new(),
                    qself: None,
                    path: syn::parse_str::<Path>("::sqlx_askama_template::askama").unwrap(),
                }),
            });
            args.push_punct(Token![,](Span::call_site()));
            args.push_value(askama_meta);
        }

        if has_source && !has_ext {
            // 添加 ext = "txt"
            let ext_meta = Meta::NameValue(syn::MetaNameValue {
                path: syn::Path::from(syn::Ident::new("ext", Span::call_site())),
                eq_token: <syn::Token![=]>::default(),
                value: syn::Expr::Lit(syn::ExprLit {
                    attrs: Vec::new(),
                    lit: syn::Lit::Str(LitStr::new("txt", Span::call_site())),
                }),
            });
            args.push_punct(Token![,](Span::call_site()));
            args.push_value(ext_meta);
        }
    }

    args
}

#[proc_macro_derive(SqlTemplate, attributes(template, add_type, ignore_type))]
pub fn sql_template(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let generics = &input.generics;
    //处理template
    let template_attrs = process_template_attr(&input);

    // 处理生命周期参数
    let (mut wrapper_generics, data_lifetime) = if let Some(lt) = generics.lifetimes().next() {
        let generics = generics.clone();
        let lt_ident = &lt.lifetime;
        (generics, quote! { #lt_ident })
    } else {
        let mut generics = generics.clone();
        let lifetime = LifetimeParam::new(syn::Lifetime::new("'q", proc_macro2::Span::call_site()));
        generics
            .params
            .insert(0, syn::GenericParam::Lifetime(lifetime));
        (generics, quote! { 'q })
    };

    // 添加DB类型参数
    wrapper_generics
        .params
        .push(syn::GenericParam::Type(syn::TypeParam {
            attrs: Vec::new(),
            ident: format_ident!("DB"),
            colon_token: None,
            bounds: syn::punctuated::Punctuated::new(),
            eq_token: None,
            default: None,
        }));

    // 收集需要绑定的类型
    let mut seen_types = BTreeSet::new();
    let mut bound_types = proc_macro2::TokenStream::new();

    // 处理字段类型
    if let syn::Data::Struct(data_struct) = &input.data {
        for field in &data_struct.fields {
            let has_ignore = field
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("ignore_type"));
            if !has_ignore {
                let ty = &field.ty;
                let ident = get_type_identifier(ty);
                if seen_types.insert(ident) {
                    bound_types.extend(quote! {
                        #ty: ::sqlx::Encode<#data_lifetime, DB> + ::sqlx::Type<DB> + #data_lifetime,
                    });
                }
            }
        }
    }

    // 处理addtype属性
    for attr in &input.attrs {
        if attr.path().is_ident("add_type") {
            if let Meta::List(meta_list) = &attr.meta {
                let parser =
                    syn::punctuated::Punctuated::<syn::Type, syn::Token![,]>::parse_terminated;
                if let Ok(types) = parser.parse2(meta_list.tokens.clone()) {
                    for ty in types {
                        let ident = get_type_identifier(&ty);
                        if seen_types.insert(ident) {
                            bound_types.extend(quote! {
                                #ty: ::sqlx::Encode<#data_lifetime, DB> + ::sqlx::Type<DB> + #data_lifetime,
                            });
                        }
                    }
                }
            }
        }
    }

    let (_impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let where_clause = where_clause.map_or_else(|| quote! { where }, |wc| quote! { #wc });
    let (wrapper_impl_generics, _, _) = wrapper_generics.split_for_impl();

    let expanded = quote! {
        impl #wrapper_impl_generics ::sqlx_askama_template::SqlTemplate<#data_lifetime, DB>
            for &#data_lifetime #name #ty_generics
            #where_clause
            DB: ::sqlx::Database,
            #bound_types
        {
            fn render_sql_with_encode_placeholder_fn(
                self,
                f: ::std::option::Option<fn(usize, &mut String)>,
                sql_buffer: &mut String,
            ) -> ::std::result::Result<
                ::std::option::Option<DB::Arguments<#data_lifetime>>,
                ::sqlx::Error,
            > {
                #[derive(::sqlx_askama_template::askama::Template)]
                #[template(#template_attrs)]
                struct Wrapper #wrapper_generics (
                    ::sqlx_askama_template::TemplateArg<#data_lifetime, DB, #name #ty_generics>
                ) #where_clause
                    DB: ::sqlx::Database,
                    #bound_types;

                impl #wrapper_impl_generics ::std::ops::Deref for Wrapper #wrapper_generics
                    #where_clause
                    DB: ::sqlx::Database,
                    #bound_types
                {
                    type Target = ::sqlx_askama_template::TemplateArg<#data_lifetime, DB, #name #ty_generics>;
                    fn deref(&self) -> &Self::Target {
                        &self.0
                    }
                }

                let mut wrapper = Wrapper(::sqlx_askama_template::TemplateArg::new(self));
                if let Some(f) = f {
                    wrapper.0.set_encode_placeholder_fn(f);
                }
                let render_res = ::sqlx_askama_template::askama::Template::render_into(&wrapper, sql_buffer)
                    .map_err(|e| ::sqlx::Error::Encode(::std::boxed::Box::new(e)))?;
                let arg = wrapper.get_arguments();
                let encode_err = wrapper.get_err();

                if let Some(e) = encode_err {
                    return ::std::result::Result::Err(e);
                }
                ::std::result::Result::Ok(arg)
            }
        }
    };

    expanded.into()
}
