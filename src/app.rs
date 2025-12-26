use leptos::prelude::*;
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::hooks::use_params_map;
use leptos_router::{
    components::{Route, Router, Routes},
    path,
};

use crate::music_data::playlist::{load_playlist, load_playlists};

const DEFAULT_TITLE: &str = "The Just Dance Archives";

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
                    <Route path=path!("/") view=HomePage />
                    <Route path=path!("/playlist/:id") view=PlayListPage />
                </Routes>
            </main>
        </Router>
    }
}

/// Renders the home page of your application.
#[component]
fn HomePage() -> impl IntoView {
    view! {
        <div class="jd-description">
            <h1>{DEFAULT_TITLE}</h1>
            <p>
                "Just Dance is a free-movement, loosely facilitated dance session in Newcastle, Australia."
            </p>
            <p>"Loosen up, let go, and have some fun. Just come and just dance!"</p>
            <p>
                "More info on the "
                <a href="https://www.facebook.com/groups/166298646868224/about">"Facebook group"</a>
            </p>

            <PlayListListPage />
        </div>
    }
}

#[component]
fn PlayListPage() -> impl IntoView {
    let params = use_params_map();
    let id = move || params.read().get("id").unwrap_or_default();
    let playlist_resource = Resource::new(move || id(), |id| async move { load_playlist(id).await });

    view! {
        <Suspense fallback=|| {
            view! { <p>"Loading..."</p> }
        }>
            {move || {
                playlist_resource
                    .get()
                    .map(|result| {
                        let playlist = result.unwrap();
                        view! { <h1>{playlist.name}</h1> }
                    })
            }}
        </Suspense>
    }
}

#[component]
fn PlayListListPage() -> impl IntoView {
    let playlists_resource = Resource::new(move || {}, |_| async move { load_playlists().await });

    let headers = vec!["Name", "Date", "Compiler(s)", "Length", "Icons"];

    view! {
        <div class="previous-set-lists">
            <h3>"Previous Playlists"</h3>
            <div class="PlayListList  page-viewtype">
                <div class="header-row">
                    {headers
                        .into_iter()
                        .map(|header| {
                            view! {
                                <div class=format!("header-cell header-{header}")>{header}</div>
                            }
                        })
                        .collect_view()}
                </div>

                <Suspense fallback=|| {
                    view! { <p>"Loading..."</p> }
                }>
                    <For
                        each=move || {
                            playlists_resource
                                .get()
                                .and_then(|r| r.ok())
                                .unwrap_or_default()
                                .into_iter()
                        }
                        key=|playlist| playlist.id.clone()
                        children=move |playlist| {
                            view! {
                                <div class="PlayList list-viewtype">
                                    <div class="item-header">
                                        <a class="name" href=format!("/playlist/{}", playlist.id)>
                                            {playlist.name}
                                        </a>
                                    </div>
                                    <div class="date">
                                        <span class="data">
                                            {crate::format_date(playlist.date)}
                                        </span>
                                    </div>
                                    <div class="compilers">
                                        <span class="data">
                                            <div class="CompilerList inline-viewtype inline-list">
                                                <For
                                                    each=move || playlist.compiler_ids.clone().into_iter()
                                                    key=|compiler_id| compiler_id.clone()
                                                    children=move |compiler_id| {
                                                        let title_text = format!("Compiler ID: {}", compiler_id);
                                                        let href_text = format!("/compiler/{}", compiler_id);
                                                        view! {
                                                            <a
                                                                title=title_text
                                                                class="Compiler inline-viewtype name"
                                                                href=href_text
                                                            >
                                                                {compiler_id}
                                                            </a>
                                                        }
                                                    }
                                                />
                                            </div>
                                        </span>
                                    </div>
                                    <div class="duration">
                                        <span class="data">
                                            {crate::format_duration(playlist.duration)}
                                            <span class="track-count">
                                                ", " {playlist.track_ids.len()} " tracks"
                                            </span>
                                        </span>
                                    </div>
                                    <div class="IconsAndLinks">
                                        <div class="wrapper"></div>
                                    </div>
                                </div>
                            }
                        }
                    />
                </Suspense>
            </div>
        </div>
    }
}
