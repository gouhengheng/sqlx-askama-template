use std::collections::BTreeSet;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::{DeriveInput, LifetimeParam, Meta, parse_macro_input};
// 用于比较类型的辅助结构
#[derive(Ord, PartialOrd, Eq, PartialEq)]
struct TypeIdentifier(String);

fn get_type_identifier(ty: &syn::Type) -> TypeIdentifier {
    TypeIdentifier(quote!(#ty).to_string())
}
/// Derive macro for generating type-safe SQL templates using Askama.
///
/// This macro generates boilerplate code to integrate Askama templates with SQLx queries,
/// providing compile-time SQL validation and parameter binding.
///
/// # Attributes
///
/// ## `#[template(...)]` (Required)
/// Defines the SQL template configuration. Accepts these parameters:
/// - `source`: Inline SQL template content (supports Askama syntax)
/// - `ext`: File extension for Askama template engine
/// - `print`: Debug output options (none|ast|code|all)
/// - `config`: Path to custom Askama configuration file
///
/// ## `#[addtype(...)]` (Optional)
/// Specifies additional type constraints for template variables:
/// - Accepts comma-separated types implementing `sqlx::Type + sqlx::Encode`
/// - Required when using non-field types in template logic
///
/// ## `#[ignore_type]` (Optional)
/// Marks struct fields to skip SQLx type validation:
/// - Use for fields that shouldn't participate in parameter binding
/// - Typically used for helper fields or complex types
///
/// # Example
/// ```
/// use sqlx_askama_template::SqlTemplate;
///
/// #[derive(SqlTemplate)]
/// #[template(
///     source = r#"
///     SELECT * FROM users
///     WHERE name = {{e(name)}}
///     AND age > {{e(min_age)}}
///     "#,
///     ext = "sql"
/// )]
/// #[addtype(i32)]
/// struct UserQuery<'a> {
///     name: &'a str,
///     #[ignore_type]
///     min_age: i32,
/// }
/// ```
///
/// # Generated Implementation
/// Implements `SqlTemplate` trait with these methods:
/// - `render_sql() -> Result<(String, Arguments<DB>)>`
/// - `render_execute() -> Result<RenderExecute<DB>>`
///
/// # Panics
/// - If required `source` attribute is missing
/// - If template syntax errors are detected at compile time
/// - If type constraints for template variables are unsatisfied
///
/// # Note
/// The generated code requires these dependencies in scope:
/// - `sqlx::{Encode, Type, Arguments}`
/// - `askama::Template`
#[proc_macro_derive(SqlTemplate, attributes(template, addtype, ignore_type))]
pub fn sql_template(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name: &syn::Ident = &input.ident;
    let generics = &input.generics;

    //let impl_generics = generics.clone();

    let wrapper_name = format_ident!("{}Wrapper", name);

    // 收集所有template属性
    let template_attrs: Vec<_> = input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("template"))
        .collect();

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
    // 使用 BTreeSet 存储唯一类型标识
    let mut seen_types = BTreeSet::new();
    // 收集需要绑定的类型
    let mut bound_types = proc_macro2::TokenStream::new();

    // 1. 处理默认绑定（非ignore_type字段）
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
    // 2. 处理addtype属性添加的类型
    for attr in &input.attrs {
        if attr.path().is_ident("addtype") {
            match &attr.meta {
                Meta::List(meta_list) => {
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
                _ => continue,
            }
        }
    }

    let (_impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let where_clause = if where_clause.is_some() {
        quote! {#where_clause}
    } else {
        quote! {where  }
    };
    let (wrapper_impl_generics, wrapper_ty_generics, _) = wrapper_generics.split_for_impl();

    let expanded = quote! {
        #[derive(::askama::Template)]
        #(#template_attrs)*
        pub struct #wrapper_name #wrapper_generics
            #where_clause
            DB: ::sqlx::Database,
            #bound_types
        {
            pub data: &#data_lifetime #name #ty_generics,
            pub arguments: ::sqlx_askama_template::TemplateArg<#data_lifetime, DB>,
        }

        impl #wrapper_impl_generics ::std::ops::Deref for #wrapper_name #wrapper_ty_generics
            #where_clause
            DB: ::sqlx::Database,
            #bound_types
        {
            type Target = &#data_lifetime #name #ty_generics;

            fn deref(&self) -> &Self::Target {
                &self.data
            }
        }

        impl #wrapper_impl_generics #wrapper_name #wrapper_ty_generics
            #where_clause
            DB: ::sqlx::Database,
            #bound_types
        {
            pub fn e<ImplEncode>(&self, arg: ImplEncode) -> ::std::string::String
            where
                ImplEncode: ::sqlx::Encode<#data_lifetime, DB> + ::sqlx::Type<DB> + #data_lifetime,
            {
                self.arguments.encode(arg)
            }

            pub fn el<ImplEncode>(
                &self,
                args: impl ::std::iter::IntoIterator<Item = ImplEncode>,
            ) -> ::std::string::String
            where
                ImplEncode: ::sqlx::Encode<#data_lifetime, DB> + ::sqlx::Type<DB> + #data_lifetime,
            {
                self.arguments.encode_list(args.into_iter())
            }

            pub fn et<ImplEncode>(&self, t: &ImplEncode) -> ::std::string::String
            where
                ImplEncode: ::sqlx::Encode<#data_lifetime, DB>
                    + ::sqlx::Type<DB>
                    + ::std::clone::Clone
                    + #data_lifetime,
            {
                self.arguments.encode(t.clone())
            }

            pub fn etl<'arg_b, ImplEncode>(
                &self,
                args: impl ::std::iter::IntoIterator<Item = &'arg_b ImplEncode>,
            ) -> ::std::string::String
            where
                #data_lifetime: 'arg_b,
                ImplEncode: ::sqlx::Encode<#data_lifetime, DB>
                    + ::sqlx::Type<DB>
                    + ::std::clone::Clone
                    + #data_lifetime,
            {
                let args = args.into_iter().cloned();
                self.arguments.encode_list(args)
            }
        }

        impl #wrapper_impl_generics ::sqlx_askama_template::SqlTemplate<#data_lifetime, DB>
            for &#data_lifetime #name #ty_generics
            #where_clause
            DB: ::sqlx::Database,
            #bound_types

        {
            fn render_sql(
                self,
            ) -> ::std::result::Result<
                (
                    ::std::string::String,
                    ::std::option::Option<DB::Arguments<#data_lifetime>>,
                ),
                ::askama::Error,
            > {
                let wrapper: #wrapper_name #wrapper_ty_generics = #wrapper_name {
                    data: self,
                    arguments: ::std::default::Default::default(),
                };

                let sql = ::askama::Template::render(&wrapper)?;
                if let ::std::option::Option::Some(e) = wrapper.arguments.get_err() {
                    return ::std::result::Result::Err(::askama::Error::Custom(e));
                }
                let arg = wrapper.arguments.get_arguments();

                ::std::result::Result::Ok((sql, arg))
            }
        }
    };

    expanded.into()
}
