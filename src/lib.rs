#![doc=include_str!("../README.md")]

use proc_macro::TokenStream;
use quote::{ToTokens, quote};
use syn::{Meta, parse_macro_input, punctuated::Punctuated};

/// Specialized version of `tracing::instrument` for recording SQLx queries to Datadog.
///
/// Accepts all arguments `tracing::instrument` accepts, but patches in extra fields.
///
/// By default, expects a function argument called `db` that has a reference to the connection, but
/// accepts a `db` parameter with an alternative identifier.
///
/// For optimal results, the `db.statement` span tag should be set to the text of the SQL query
/// executed.
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
///     let query = sqlx::query_as("SELECT name, email FROM users WHERE id = ? LIMIT 1");
///     tracing::Span::current().record("db.statement", query.sql().trim());
///     query.bind(user_id).fetch_one(db).await
/// }
/// ```
#[proc_macro_attribute]
pub fn instrument_query(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args with Punctuated::<Meta, syn::Token![,]>::parse_terminated);
    let mut input_fn = parse_macro_input!(item as syn::ItemFn);

    let mut instrument_args: Vec<Meta> = vec![];
    let mut fields = vec![];
    let mut db_ident = quote! { db };

    for arg in args {
        if let Meta::NameValue(name_value) = arg.clone() {
            if name_value.path.get_ident().unwrap() == "db" {
                db_ident = name_value.value.into_token_stream();
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

    // These are in reverse.
    let injected_tags = vec![
        quote! { ::tracing::Span::current().record("peer.service", #db_ident.connect_options().get_database()); },
        quote! { ::tracing::Span::current().record("peer.hostname", #db_ident.connect_options().get_host()); },
        quote! { ::tracing::Span::current().record("out.host", #db_ident.connect_options().get_host()); },
        quote! { ::tracing::Span::current().record("out.port", #db_ident.connect_options().get_port()); },
        quote! { ::tracing::Span::current().record("db.instance", #db_ident.connect_options().get_database()); },
        quote! { ::tracing::Span::current().record("db.name", #db_ident.connect_options().get_database()); },
        quote! { ::tracing::Span::current().record("db.system", #db_ident.connect_options().to_url_lossy().scheme().replace("postgres", "postgresql")); },
        quote! { use ::sqlx::ConnectOptions; },
    ];
    for tag in injected_tags {
        input_fn
            .block
            .stmts
            .insert(0, syn::parse(tag.into()).unwrap());
    }

    let instrument_attr = quote! {
        #[::tracing::instrument(
            fields(
                span.kind = "client",
                span.type = "sql",
                component = "sqlx"
                operation = "sqlx.query",
                peer.hostname,
                peer.service,
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

    // TODO Inject propagation comment into query.
    // Format is /*key=value,key=value*/
    // keys are:
    // dde (environment)
    // ddps (parent service)
    // ddpv (parent version)
    // ddh (db peer host)
    // dddb (db instance)
    // traceparent (span id)

    let output = quote! {
        #instrument_attr
        #input_fn
    };

    TokenStream::from(output)
}
