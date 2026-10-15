// Compiles `static/input.css` with the standalone Tailwind CLI. The result is
// bundled as a Topcoat asset and linked from the layout via
// `topcoat::tailwind::stylesheet!()` - no Node.js, no committed binary, no
// committed output CSS.
//
// By default Topcoat downloads (and caches) a pinned Tailwind CLI release.
// Set TAILWIND_CLI to a local `tailwindcss` executable to skip the download
// (offline builds, CI caches, networks with TLS interception).
fn main() {
    let mut config = topcoat::tailwind::BuildConfig::new().input("static/input.css");
    if let Some(cli) = std::env::var_os("TAILWIND_CLI") {
        config = config.executable(cli);
    }
    config.render().expect("failed to compile Tailwind CSS");
}
