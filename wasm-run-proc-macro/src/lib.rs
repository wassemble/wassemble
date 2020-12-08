//! See the crate `wasm-run` for documentation.

mod attr_parser;
mod main_generator;

use cargo_metadata::MetadataCommand;
use proc_macro::TokenStream;
use syn::{parse_macro_input, ItemEnum};

/// Makes an entrypoint to your binary (not WASM).
///
/// It requires to be used with `structopt` on an `enum`. Please consult the documentation of
/// `structopt` if you don't know how to make an `enum` with it.
///
/// By default it provides a command `Build` and a command `Serve` which you can override simply by
/// providing them manually. Otherwise it uses the defaults (`DefaultBuildArgs` and
/// `DefaultServeArgs`).
///
/// There are a number of named arguments you can provide to the macro:
///  -  `other_cli_commands`: a function that is called if you have added new commands to the
///     `enum`;
///  -  `pre_build`: a function that is called when the build has not started yet (you can tweak
///     the command-line arguments of the build command);
///  -  `post_build`: a function that is called when the build is finished (after the optimization
///     with `wasm-opt`);
///  -  `watch`: a function that is called when the watcher is being initialized (allowing you to
///     add extra things to watch for example);
///  -  `serve`: (only if built with the `serve` feature): a function that is called when the HTTP
///     serve is getting configured;
///  -  `run_server`: (only if built *without* the `serve` feature): a function that is called to
///     run the HTTP server;
///  -  `default_build_path`: a function that is called that provides the default directory path
///     when the user didn't provide it through the command-line arguments (the default is
///     `workspace root/build`).
///
/// You can also change the package that is built by providing its name in the first positional
/// argument:
///
/// ```ignore
/// #[wasm_run::main("my-frontend-crate")]
/// ```
///
/// # Example
///
/// ```
/// use anyhow::Result;
/// use std::fs;
/// use std::path::PathBuf;
/// use structopt::StructOpt;
/// use wasm_run::prelude::*;
///
/// /// Makes an entrypoint to your binary.
/// #[wasm_run::main(
///     "basic",
///     other_cli_commands = run_other_cli_commands,
///     pre_build = pre_build,
///     post_build = post_build,
///     serve = serve,
///     watch = watch,
///     default_build_path = default_build_path,
/// )]
/// #[derive(StructOpt, Debug)]
/// enum Cli {
///     Build(BuildCommand),
///     Serve(ServeCommand),
///     Hello,
/// }
///
/// /// Define a custom `build` command.
/// #[derive(StructOpt, Debug)]
/// struct BuildCommand {
///     #[structopt(skip)]
///     i: i32,
/// }
///
/// impl BuildArgs for BuildCommand {
///     fn build_path(&self) -> &PathBuf {
///         self.default_build_path()
///     }
///
///     fn profiling(&self) -> bool {
///         false
///     }
/// }
///
/// /// Define a custom `serve` command.
/// #[derive(StructOpt, Debug)]
/// struct ServeCommand {
///     #[structopt(flatten)]
///     build_args: BuildCommand,
///
///     #[structopt(skip)]
///     j: i32,
/// }
///
/// impl ServeArgs for ServeCommand {
///     fn build_args(&self) -> &dyn BuildArgs {
///         &self.build_args
///     }
///
///     fn log(&self) -> bool {
///         false
///     }
///
///     fn ip(&self) -> &str {
///         "127.0.0.1"
///     }
///
///     fn port(&self) -> u16 {
///         3000
///     }
/// }
///
/// /// This function is called if you have added new commands to the enum.
/// fn run_other_cli_commands(cli: Cli, _metadata: &Metadata, _package: &Package) -> Result<()> {
///     match cli {
///         Cli::Build(_) | Cli::Serve(_) => unreachable!(),
///         Cli::Hello => println!("Hello World!"),
///     }
///
///     Ok(())
/// }
///
/// /// This function is called after the build.
/// fn pre_build(
///     args: &BuildCommand,
///     profile: BuildProfile,
///     command: &mut std::process::Command,
/// ) -> Result<()> {
///     let _i = args.i;
///
///     command
///         .arg("--no-default-features")
///         .env("RUSTFLAGS", "-Zmacro-backtrace");
///
///     Ok(())
/// }
///
/// /// This function is called after the build.
/// fn post_build(
///     args: &BuildCommand,
///     _profile: BuildProfile,
///     wasm_js: String,
///     wasm_bin: Vec<u8>,
/// ) -> Result<()> {
///     let _i = args.i;
///
///     let build_path = args.build_path();
///     fs::write(build_path.join("app.js"), wasm_js)?;
///     fs::write(build_path.join("app_bg.wasm"), wasm_bin)?;
///     fs::write(
///         build_path.join("index.html"),
///         "<html><body>Custom index.html</body>",
///     )?;
///
///     Ok(())
/// }
///
/// /// This function is called before serving files.
/// fn serve(args: &ServeCommand, server: &mut Server<()>) -> Result<()> {
///     let _j = args.j;
///
///     use tide::{Body, Response};
///
///     let build_path = args.build_args().build_path();
///     let index_path = build_path.join("index.html");
///
///     server.at("/").serve_dir(args.build_args().build_path())?;
///     server.at("/").get(move |_| {
///         let index_path = index_path.clone();
///         async move { Ok(Response::from(Body::from_file(index_path).await?)) }
///     });
///
///     Ok(())
/// }
///
/// /// This function is called when the watcher is being initialized.
/// fn watch(args: &ServeCommand, watcher: &mut RecommendedWatcher) -> Result<()> {
///     let _j = args.j;
///
///     use notify::{RecursiveMode, Watcher};
///     use std::collections::HashSet;
///     use std::iter::FromIterator;
///
///     let metadata = args.build_args().metadata();
///
///     let _ = watcher.watch("index.html", RecursiveMode::Recursive);
///
///     let members: HashSet<_> = HashSet::from_iter(&metadata.workspace_members);
///
///     for package in metadata.packages.iter().filter(|x| members.contains(&x.id)) {
///         let _ = watcher.watch(&package.manifest_path, RecursiveMode::Recursive);
///         let _ = watcher.watch(
///             package.manifest_path.parent().unwrap().join("src"),
///             RecursiveMode::Recursive,
///         );
///     }
///
///     Ok(())
/// }
///
/// /// Define another build path if not provided by the user in the command-line arguments.
/// fn default_build_path(metadata: &Metadata, _package: &Package) -> PathBuf {
///     metadata.workspace_root.join("build")
/// }
/// ```
#[proc_macro_attribute]
pub fn main(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemEnum);
    let attr = parse_macro_input!(attr with attr_parser::Attr::parse);
    let metadata = MetadataCommand::new()
        .exec()
        .expect("could not get metadata");

    main_generator::generate(item, attr, &metadata)
        .unwrap()
        .into()
}
