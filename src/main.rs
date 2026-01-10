use dioxus::prelude::*;
use tracing::Level;

mod components;
mod music_data;
mod views;
use views::compiler::{CompilerComp, CompilerListComp};
use views::home::Home;
use views::navbar::Navbar;
use views::playlist::PlaylistComp;
use views::track::TrackComp;

mod util;

#[cfg(feature = "server")]
mod server;

/// The Route enum is used to define the structure of internal routes in our app. All route enums need to derive
/// the [`Routable`] trait, which provides the necessary methods for the router to work.
/// 
/// Each variant represents a different URL pattern that can be matched by the router. If that pattern is matched,
/// the components for that route will be rendered.
#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    // The layout attribute defines a wrapper for all routes under the layout. Layouts are great for wrapping
    // many routes with a common UI like a navbar.
    #[layout(Navbar)]
        #[route("/")]
        Home {},
        #[route("/playlist/:id")]
        PlaylistComp { id: String },
        #[route("/compiler")]
        CompilerListComp {},
        #[route("/compiler/:id")]
        CompilerComp { id: String },
        #[route("/track/:id")]
        TrackComp { id: String },
}

// We can import assets in dioxus with the `asset!` macro. This macro takes a path to an asset relative to the crate root.
// The macro returns an `Asset` type that will display as the path to the asset in the browser or a local path in desktop bundles.
const FAVICON: Asset = asset!("/assets/favicon.ico");
// The asset macro also minifies some assets like CSS and JS to make bundled smaller
const MAIN_CSS: Asset = asset!("/assets/styling/main.scss");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

fn main() {
    dioxus::logger::init(Level::INFO).expect("failed to init logger");

    #[cfg(feature = "server")]
    dioxus::serve(|| async move {
        // Create a new axum router for our Dioxus app
        let router = dioxus::server::router(App);

        crate::music_data::server::create_indexes::<music_data::compiler::Compiler>()
            .await
            .unwrap();

        // And then return it
        Ok(router)
    });

    #[cfg(not(feature = "server"))]
    dioxus::launch(App);
}

/// App is the main component of our app. Components are the building blocks of dioxus apps. Each component is a function
/// that takes some props and returns an Element. In this case, App takes no props because it is the root of our app.
///
/// Components should be annotated with `#[component]` to support props, better error messages, and autocomplete
#[component]
fn App() -> Element {
    let compilers: Resource<music_data::MusicItemsById<music_data::compiler::Compiler>> =
        use_resource(|| async move {
            let items = music_data::compiler::load_compilers()
                .await
                .unwrap_or_default();
            music_data::MusicItemsById::from(items)
        });
    use_context_provider(|| compilers);

    let playlists: Resource<music_data::MusicItemsById<music_data::playlist::PlayList>> =
        use_resource(|| async move {
            let items = music_data::playlist::load_playlists()
                .await
                .unwrap_or_default();
            music_data::MusicItemsById::from(items)
        });
    use_context_provider(|| playlists);

    rsx! {
        // In addition to element and text (which we will see later), rsx can contain other components. In this case,
        // we are using the `document::Link` component to add a link to our favicon and main CSS file into the head of our app.
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }

        // The router component renders the route enum we defined above. It will handle synchronization of the URL and render
        // the layouts and components for the active route.
        Router::<Route> {}
    }
}
