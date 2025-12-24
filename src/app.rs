use leptos::prelude::*;
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::{
    StaticSegment,
    components::{Route, Router, Routes},
};

use crate::load_playlist;

const DEFAULT_TITLE: &str = "Playl!st - the Just Dance archive";

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <AutoReload options=options.clone() />
                <HydrationScripts options />
                <MetaTags />
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    view! {
        // injects a stylesheet into the document <head>
        // id=leptos means cargo-leptos will hot-reload this stylesheet
        <Stylesheet id="leptos" href="/pkg/playlistv2.css" />

        // sets the document title
        <Title text=DEFAULT_TITLE />

        // content for this welcome page
        <Router>
            <main>
                <Routes fallback=|| "Page not found.".into_view()>
                    <Route path=StaticSegment("") view=HomePage />
                </Routes>
            </main>
        </Router>
    }
}

/// Renders the home page of your application.
#[component]
fn HomePage() -> impl IntoView {
    view! {
        <h1>{DEFAULT_TITLE}</h1>
        <PlayListPage />
    }
}

#[component]
fn PlayListPage() -> impl IntoView {
    let playlist_resource = Resource::new(|| (), |_| async { load_playlist().await });
    
    view! {
        <Suspense fallback=|| view! { <p>"Loading..."</p> }>
            {move || {
                playlist_resource.get().map(|result| {
                    let playlist_name = result.unwrap_or("Error loading playlist".to_string());
                    view! {
                        <h1>"Playlist Page"</h1>
                        <p>{playlist_name}</p>
                    }
                })
            }}
        </Suspense>
    }
}