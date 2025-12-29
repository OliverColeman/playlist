use std::collections::HashMap;

use leptos::prelude::*;
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::hooks::use_params_map;
use leptos_router::{
    components::{Route, Router, Routes},
    path,
};

use leptos::either::Either;

use crate::music_data::compiler::{Compiler, load_compilers};
use crate::music_data::playlist::{PlayList, load_playlists};
use crate::music_data::track::load_playlist_tracks;

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

const DEFAULT_TITLE: &str = "The Just Dance Archives";

fn key_music_items_by_id<T: crate::music_data::MusicItem>(
    items: Result<Vec<T>, crate::AppError>,
) -> HashMap<String, T> {
    items
        .map(|list| {
            list.into_iter()
                .map(|item| (item.id().to_string(), item))
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default()
}

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    let playlists_by_id_resource = Resource::new(
        move || {},
        |_| async move { key_music_items_by_id(load_playlists().await) },
    );
    provide_context(playlists_by_id_resource);

    let compilers_by_id_resource = Resource::new(
        move || {},
        |_| async move { key_music_items_by_id(load_compilers().await) },
    );
    provide_context(compilers_by_id_resource);

    view! {
        // id=leptos means cargo-leptos will hot-reload this stylesheet
        <Stylesheet id="leptos" href="/pkg/playlistv2.css" />

        <Title text=DEFAULT_TITLE />

        <div class="jd-description">
            <h1>{DEFAULT_TITLE}</h1>
            <p>
                "Just Dance is a free-movement, loosely facilitated dance session in Newcastle, Australia."
                " Loosen up, let go, and just dance! More info on the "
                <a href="https://www.facebook.com/groups/166298646868224/about">"Facebook group"</a>
            </p>
        </div>

        <Router>
            <main>
                <Routes fallback=|| "Page not found.".into_view()>
                    <Route path=path!("/") view=PlayListListComp />
                    <Route path=path!("/playlist/:id") view=PlayListComp />
                    <Route path=path!("/compiler") view=CompilerListComp />
                // <Route path=path!("/compiler/:id") view=CompilerPage />
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn Loading() -> impl IntoView {
    view! { <span class="loading" /> }
}

#[component]
fn PlayListListComp() -> impl IntoView {
    let playlists_resource = expect_context::<Resource<HashMap<String, PlayList>>>();

    let headers = vec!["Name", "Date", "Compiler(s)", "Length", "Icons"];

    view! {
        <div class="previous-set-lists">
            <h2>"Previous Playlists"</h2>
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
                    view! { <Loading /> }
                }>
                    <For
                        each=move || {
                            let mut playlists: Vec<PlayList> = playlists_resource
                                .get()
                                .map(|map| map.into_values().collect::<Vec<PlayList>>())
                                .unwrap_or_default();
                            playlists
                                .sort_by_key(|playlist| -playlist.date.unwrap_or_default() as i64);
                            playlists.into_iter()
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
                                        <div class="data">{crate::format_date(playlist.date)}</div>
                                    </div>
                                    <div class="compilers">
                                        <CompilerListInlineComp compiler_ids=playlist
                                            .compiler_ids
                                            .clone() />
                                    </div>
                                    <div class="duration">
                                        <div class="data">
                                            {crate::format_duration(playlist.duration)}
                                            <span class="track-count">
                                                ", " {playlist.track_ids.len()} " tracks"
                                            </span>
                                        </div>
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

#[component]
fn PlayListComp() -> impl IntoView {
    let params = use_params_map();
    let id = move || params.read().get("id").unwrap_or_default();
    let playlist_resource = expect_context::<Resource<HashMap<String, PlayList>>>();

    view! {
        <Suspense fallback=|| {
            view! { <Loading /> }
        }>
            {move || {
                playlist_resource
                    .get()
                    .map(|playlists| {
                        let playlist = playlists.get(&id());
                        match playlist {
                            None => Either::Right(view! { <h3>"Not found!"</h3> }),
                            Some(playlist) => {
                                Either::Left(
                                    view! {
                                        <div class="PlayList page-viewtype">
                                            <div class="item-details" style="width: 100%;">
                                                <div class="item-header">
                                                    <div class="name">{playlist.name.clone()}</div>
                                                </div>
                                                <div class="date">
                                                    <span class="label label-default">"Date: "</span>
                                                    <div class="data">{crate::format_date(playlist.date)}</div>
                                                </div>
                                                <div class="compilers">
                                                    <span class="label label-default">"Created by: "</span>
                                                    <CompilerListInlineComp compiler_ids=playlist
                                                        .compiler_ids
                                                        .clone() />
                                                </div>
                                                <div class="duration">
                                                    <span class="label label-default">Duration:</span>
                                                    <div class="data">
                                                        {crate::format_duration(playlist.duration)}, " "
                                                        {playlist.track_ids.len()} " tracks"
                                                    </div>
                                                </div>
                                                <div class="notes">
                                                    <span class="label label-default">Notes:</span>
                                                    <div class="data">
                                                        <pre style="width: 100%;">
                                                            {playlist
                                                                .notes
                                                                .clone()
                                                                .unwrap_or_else(|| "[No notes set]".to_string())}
                                                        </pre>
                                                    </div>
                                                </div>
                                            </div>
                                            <div class="tracks-wrapper">
                                                <TrackListComp track_ids=playlist.track_ids.clone() />
                                            </div>
                                        </div>
                                    },
                                )
                            }
                        }
                    })
            }}
        </Suspense>
    }
}

#[component]
fn TrackListComp(track_ids: Vec<String>) -> impl IntoView {
    let track_ids = StoredValue::new(track_ids);
    let track_ids_for_resource = track_ids.get_value();
    let tracks_resource = Resource::new(
        move || {},
        move |_| {
            let track_ids = track_ids_for_resource.clone();
            async move { key_music_items_by_id(load_playlist_tracks(track_ids).await) }
        },
    );

    let headers = vec!["Title", "Artist", "Album", "Length", "Last list", "Icons"];

    view! {
        <div class="TrackList">
            <div class="header-row list-compact-viewtype">
                {headers
                    .into_iter()
                    .map(|header| {
                        view! { <div class=format!("header-cell header-{header}")>{header}</div> }
                    })
                    .collect_view()}
            </div>
            <Suspense fallback=|| {
                view! { <Loading /> }
            }>
                {move || {
                    tracks_resource
                        .get()
                        .map(|tracks_map| {
                            let track_ids_list = track_ids.get_value();
                            view! {
                                <For
                                    each=move || {
                                        track_ids_list
                                            .clone()
                                            .into_iter()
                                            .filter_map({
                                                let value = tracks_map.clone();
                                                move |track_id| { value.get(&track_id).cloned() }
                                            })
                                    }
                                    key=|track| track.id.clone()
                                    children=move |track| {
                                        view! {
                                            <div class="Track list-compact-viewtype">
                                                <a
                                                    class="Track inline-viewtype name"
                                                    href=format!("/track/{}", track.id)
                                                >
                                                    {track.name.clone()}
                                                </a>
                                                <div class="artists inline-list">
                                                    <div class="data">{track.artist_ids.join(", ")}</div>
                                                </div>
                                                <a class="album inline-viewtype name">
                                                    {track
                                                        .album_id
                                                        .clone()
                                                        .unwrap_or_else(|| "<Unknown>".to_string())}

                                                </a>
                                                <div class="duration">
                                                    {crate::format_duration(track.duration.unwrap_or_default())}
                                                </div>
                                                <a class="PlayList inline-viewtype name last-list" href="#">
                                                    "TODO"
                                                </a>
                                                <div class="IconsAndLinks">"TODO"</div>
                                            </div>
                                        }
                                    }
                                />
                            }
                        })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn CompilerListInlineComp(compiler_ids: Vec<String>) -> impl IntoView {
    let compilers_resource = expect_context::<Resource<HashMap<String, Compiler>>>();
    view! {
        <div class="data">
            <div class="CompilerList inline-viewtype inline-list">
                <Suspense fallback=|| view! { <span class="loading" /> }>
                    <For
                        each=move || compiler_ids.clone().into_iter()
                        key=|compiler_id| compiler_id.clone()
                        children=move |compiler_id| {
                            let compilers_by_id = compilers_resource.get().unwrap_or_default();
                            let compiler_name = compilers_by_id
                                .get(&compiler_id)
                                .map(|c| c.name.clone())
                                .unwrap_or_else(|| "<Unknown>".to_string());
                            let title_text = compiler_name.clone();
                            let href_text = format!("/compiler/{}", compiler_id);
                            view! {
                                <a
                                    title=title_text
                                    class="Compiler inline-viewtype name"
                                    href=href_text
                                >
                                    {compiler_name}
                                </a>
                            }
                        }
                    />
                </Suspense>
            </div>
        </div>
    }
}

#[component]
#[allow(dead_code)]
fn CompilerListComp() -> impl IntoView {
    let compilers_resource = expect_context::<Resource<HashMap<String, Compiler>>>();

    let headers = vec!["Name", "Playlists", "Icons"];

    view! {
        <div class="CompilerList">
            <h2>"Compilers"</h2>
            <div class="compilers">
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
                    view! { <Loading /> }
                }>
                    <For
                        each=move || {
                            let compilers_map = compilers_resource.get().unwrap_or_default();
                            let mut compilers: Vec<Compiler> = compilers_map
                                .into_values()
                                .collect();
                            compilers.sort_by_key(|compiler| compiler.name.clone());
                            compilers.into_iter()
                        }
                        key=|compiler| compiler.id.clone()
                        children=move |compiler| {
                            view! {
                                <div class="Compiler list-viewtype">
                                    <a
                                        class="Compiler inline-viewtype name"
                                        href=format!("/compiler/{}", compiler.id)
                                    >
                                        {compiler.name}
                                    </a>
                                    <CompilerPlayListListComp compiler_id=compiler.id />
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

#[component]
fn CompilerPlayListListComp(compiler_id: String) -> impl IntoView {
    let playlists_resource = expect_context::<Resource<HashMap<String, PlayList>>>();

    view! {
        <div class="PlayListList inline-viewtype inline-list">
            <Suspense fallback=|| view! { <Loading /> }>
                <For
                    each=move || {
                        let playlists_map = playlists_resource.get().unwrap_or_default();
                        let mut playlists: Vec<PlayList> = playlists_map.into_values().collect();
                        let compiler_id = compiler_id.clone();
                        playlists = playlists
                            .into_iter()
                            .filter(move |playlist| {
                                playlist.compiler_ids.contains(&compiler_id)
                            })
                            .collect::<Vec<PlayList>>();
                        playlists.sort_by_key(|playlist| -playlist.date.unwrap_or_default() as i64);
                        playlists.into_iter()
                    }
                    key=|playlist| playlist.id.clone()
                    children=move |playlist| {
                        view! {
                            <a
                                class="PlayList inline-viewtype name"
                                href=format!("/playlist/{}", playlist.id)
                            >
                                {playlist.name.clone()}
                            </a>
                        }
                    }
                />
            </Suspense>
        </div>
    }
}
