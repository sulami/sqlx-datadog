#![doc=include_str!("../README.md")]

use proc_macro::TokenStream;
use quote::{ToTokens, quote};
use syn::{Meta, parse_macro_input, punctuated::Punctuated};

/// Specialized version of `tracing::instrument` for recording SQLx queries to Datadog.
///
/// Accepts all arguments `tracing::instrument` accepts, but patches in extra fields.
///
/// By default, expects a function argument called `db` that has a reference to the database
/// connection.
///
/// If there is a literal string binding called `query` present, its value will be used to set the
/// relevant span tags.
///
/// The names of the connection and query binding can be changed using macro parameters, e.g.:
///
/// ```
/// # #[macro_use] extern crate sqlx_datadog;
/// # use sqlx::Execute;
/// #
/// # #[derive(Debug, sqlx::FromRow)]
/// # struct User { name: String, email: String }
/// #
/// #[instrument_query(skip(conn), db = conn, query = my_query)]
/// async fn fetch_user(conn: &sqlx::MySqlPool, user_id: i64) -> Result<User, sqlx::Error> {
///     let my_query = "SELECT name, email FROM users WHERE id = ? LIMIT 1";
///     sqlx::query_as(my_query).bind(user_id).fetch_one(conn).await
/// }
/// ```
///
/// # Example
///
/// ```
/// # #[macro_use] extern crate sqlx_datadog;
/// # use sqlx::Execute;
/// #
/// # #[derive(Debug, sqlx::FromRow)]
/// # struct User { name: String, email: String }
/// #
/// #[instrument_query(skip(db))]
/// async fn fetch_user(db: &sqlx::MySqlPool, user_id: i64) -> Result<User, sqlx::Error> {
///     let query = "SELECT name, email FROM users WHERE id = ? LIMIT 1";
///     sqlx::query_as(query).bind(user_id).fetch_one(db).await
/// }
/// ```
#[proc_macro_attribute]
pub fn instrument_query(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args with Punctuated::<Meta, syn::Token![,]>::parse_terminated);
    let mut input_fn = parse_macro_input!(item as syn::ItemFn);

    let mut instrument_args: Vec<Meta> = vec![];
    let mut fields = vec![];
    let mut db_ident = quote! { db };
    let mut query_ident = quote! { query };

    for arg in args {
        if let Meta::NameValue(name_value) = arg.clone() {
            if name_value.path.get_ident().unwrap() == "db" {
                db_ident = name_value.value.into_token_stream();
            } else if name_value.path.get_ident().unwrap() == "query" {
                query_ident = name_value.value.into_token_stream();
            } else {
                instrument_args.push(arg);
            }
        } else if let Meta::List(list_value) = arg.clone() {
            if list_value.path.get_ident().unwrap() == "fields" {
                fields.extend(list_value.tokens);
            } else {
                instrument_args.push(arg);
            }
        } else {
            instrument_args.push(arg);
        }
    }

    // Find the query text.
    let mut query_literal = None;
    for stmt in input_fn.block.stmts.iter() {
        if let syn::Stmt::Local(local) = stmt &&
            let syn::Pat::Ident(pat_ident) = &local.pat &&
            pat_ident.ident == query_ident.to_string() &&
            let Some(init) = &local.init &&
            let syn::Expr::Lit(expr_lit) = &*init.expr &&
            let syn::Lit::Str(lit_str) = &expr_lit.lit {
                // Save original for span tags
                query_literal = Some(lit_str.clone());
                break;
        }
    }

    // These are in reverse.
    let mut injected_tags = vec![
        quote! { ::tracing::Span::current().record("peer.hostname", #db_ident.connect_options().get_host()); },
        quote! { ::tracing::Span::current().record("out.host", #db_ident.connect_options().get_host()); },
        quote! { ::tracing::Span::current().record("out.port", #db_ident.connect_options().get_port()); },
        quote! { ::tracing::Span::current().record("db.instance", #db_ident.connect_options().get_database()); },
        quote! { ::tracing::Span::current().record("db.name", #db_ident.connect_options().get_database()); },
        quote! { ::tracing::Span::current().record("db.system", #db_ident.connect_options().to_url_lossy().scheme().replace("postgres", "postgresql")); },
        quote! { use ::sqlx::ConnectOptions; },
    ];

    // If we know the query text, inject it into the span tags.
    if let Some(query_lit) = query_literal {
        injected_tags.insert(0, quote! { ::tracing::Span::current().record("db.statement", #query_lit.trim()); });
        injected_tags.insert(0, quote! { ::tracing::Span::current().record("resource", #query_lit.trim()); });
    }

    for tag in injected_tags {
        input_fn.block.stmts.insert(0, syn::parse(tag.into()).unwrap());
    }

    let instrument_attr = quote! {
        #[::tracing::instrument(
            fields(
                span.kind = "client",
                span.type = "sql",
                component = "sqlx",
                operation = "sqlx.query",
                resource,
                peer.hostname,
                out.host,
                out.port,
                db.system,
                db.instance,
                db.name,
                db.statement,
                #(#fields),*
            )
            #(#instrument_args),*
        )]
    };

    let output = quote! {
        #instrument_attr
        #input_fn
    };

    TokenStream::from(output)
}
