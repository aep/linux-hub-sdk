//! # Rocket - Code Generation
//!
//! This crate implements the code generation portions of Rocket. This includes
//! custom derives, custom attributes, and procedural macros. The documentation
//! here is purely technical. The code generation facilities are documented
//! thoroughly in the [Rocket programming guide](https://rocket.rs/guide).
//!
//! ## Custom Attributes
//!
//! This crate implements the following custom attributes:
//!
//!   * **route**
//!   * **get**
//!   * **put**
//!   * **post**
//!   * **delete**
//!   * **head**
//!   * **patch**
//!   * **options**
//!   * **error**
//!
//! The grammar for all _route_ attributes, including **route**, **get**,
//! **put**, **post**, **delete**, **head**, **patch**, and **options** is
//! defined as:
//!
//! <pre>
//! route := METHOD? '(' ('path' '=')? path (',' kv_param)* ')'
//!
//! path := URI_SEG
//!       | DYNAMIC_PARAM
//!       | '?' DYNAMIC_PARAM
//!       | path '/' path
//!       (string literal)
//!
//! kv_param := 'rank' '=' INTEGER
//!           | 'format' '=' STRING
//!           | 'data' '=' DYNAMIC_PARAM
//!
//! INTEGER := isize, as defined by Rust
//! STRING := UTF-8 string literal, as defined by Rust
//! IDENT := valid identifier, as defined by Rust
//!
//! URI_SEG := valid HTTP URI Segment
//! DYNAMIC_PARAM := '<' IDENT '..'? '>' (string literal)
//! </pre>
//!
//! Note that the **route** attribute takes a method as its first argument,
//! while the remaining do not. That is, **route** looks like:
//!
//!     #[route(GET, path = "/hello")]
//!
//! while the equivalent using **get** looks like:
//!
//!     #[get("/hello")]
//!
//! The syntax for the **error** attribute is:
//!
//! <pre>
//! error := INTEGER
//! </pre>
//!
//! A use of the `error` attribute looks like:
//!
//!     #[error(404)]
//!
//! ## Custom Derives
//!
//! This crate implements the following custom derives:
//!
//!   * **FromForm**
//!
//! ### `FromForm`
//!
//! The [`FromForm`] derive can be applied to structures with named fields:
//!
//!     #[derive(FromForm)]
//!     struct MyStruct {
//!         field: usize,
//!         other: String
//!     }
//!
//! Each field's type is required to implement [`FromFormValue`]. The derive
//! accepts one field attribute: `form`, with the following syntax:
//!
//! <pre>
//! form := 'field' '=' '"' IDENT '"'
//!
//! IDENT := valid identifier, as defined by Rust
//! </pre>
//!
//! When applied, the attribute looks as follows:
//!
//!     #[derive(FromForm)]
//!     struct MyStruct {
//!         field: usize,
//!         #[form(field = "renamed_field")]
//!         other: String
//!     }
//!
//! The derive generates an implementation for the [`FromForm`] trait. The
//! implementation parses a form whose field names match the field names of the
//! structure on which the derive was applied. Each field's value is parsed with
//! the [`FromFormValue`] implementation of the field's type. The `FromForm`
//! implementation succeeds only when all of the field parses succeed.
//!
//! The `form` field attribute can be used to direct that a different incoming
//! field name is expected. In this case, the attribute's field name is used
//! instead of the structure's field name when parsing a form.
//!
//! [`FromForm`]: /rocket/request/trait.FromForm.html
//! [`FromFormValue`]: /rocket/request/trait.FromFormValue.html
//!
//! ## Procedural Macros
//!
//! This crate implements the following procedural macros:
//!
//!   * **routes**
//!   * **errors**
//!
//! The syntax for both of these is defined as:
//!
//! <pre>
//! macro := PATH (',' macro)*
//!
//! PATH := a path, as defined by Rust
//! </pre>
//!
//! # Debugging Codegen
//!
//! When the `ROCKET_CODEGEN_DEBUG` environment variable is set, this crate logs
//! the items it has generated to the console at compile-time. For example, you
//! might run the following to build a Rocket application with codegen logging
//! enabled:
//!
//! ```
//! ROCKET_CODEGEN_DEBUG=1 cargo build
//! ```

#![crate_type = "dylib"]
#![feature(quote, concat_idents, plugin_registrar, rustc_private)]
#![feature(custom_attribute)]
#![feature(i128_type)]
#![allow(unused_attributes)]
#![allow(deprecated)]

#[macro_use] extern crate log;
extern crate syntax;
extern crate syntax_ext;
extern crate rustc_plugin;
extern crate rocket;

#[macro_use] mod utils;
mod parser;
mod macros;
mod decorators;

use std::env;
use rustc_plugin::Registry;
use syntax::ext::base::SyntaxExtension;
use syntax::symbol::Symbol;

const DEBUG_ENV_VAR: &'static str = "ROCKET_CODEGEN_DEBUG";

const PARAM_PREFIX: &'static str = "rocket_param_";
const ROUTE_STRUCT_PREFIX: &'static str = "static_rocket_route_info_for_";
const CATCH_STRUCT_PREFIX: &'static str = "static_rocket_catch_info_for_";
const ROUTE_FN_PREFIX: &'static str = "rocket_route_fn_";
const CATCH_FN_PREFIX: &'static str = "rocket_catch_fn_";

const ROUTE_ATTR: &'static str = "rocket_route";
const ROUTE_INFO_ATTR: &'static str = "rocket_route_info";

const CATCHER_ATTR: &'static str = "rocket_catcher";

macro_rules! register_decorators {
    ($registry:expr, $($name:expr => $func:ident),+) => (
        $($registry.register_syntax_extension(Symbol::intern($name),
                SyntaxExtension::MultiModifier(Box::new(decorators::$func)));
         )+
    )
}

macro_rules! register_derives {
    ($registry:expr, $($name:expr => $func:ident),+) => (
        $($registry.register_custom_derive(Symbol::intern($name),
                SyntaxExtension::MultiDecorator(Box::new(decorators::$func)));
         )+
    )
}

/// Compiler hook for Rust to register plugins.
#[plugin_registrar]
pub fn plugin_registrar(reg: &mut Registry) {
    // Enable logging early if the DEBUG_ENV_VAR is set.
    if env::var(DEBUG_ENV_VAR).is_ok() {
        ::rocket::logger::init(::rocket::config::LoggingLevel::Debug);
    }

    reg.register_macro("routes", macros::routes);
    reg.register_macro("errors", macros::errors);

    register_derives!(reg,
        "derive_FromForm" => from_form_derive
    );

    register_decorators!(reg,
        "error" => error_decorator,
        "route" => route_decorator,
        "get" => get_decorator,
        "put" => put_decorator,
        "post" => post_decorator,
        "delete" => delete_decorator,
        "head" => head_decorator,
        "patch" => patch_decorator,
        "options" => options_decorator
    );
}
