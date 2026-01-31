use crate::components;
use crate::views::artist::ArtistListComp;
use crate::views::track::TrackListComp;
use dioxus::prelude::*;
use std::time::Duration;

use playlist_core::models;

#[component]
pub fn SearchComp() -> Element {
    let mut search_input = use_signal(|| "".to_string());
    let mut debounced_search = use_signal(|| "".to_string());
    let mut generation = use_signal(|| 0u32);
    let mut selected_music_item_type = use_signal(|| "Tracks".to_string());

    let compilers_by_id =
        use_context::<Resource<models::MusicItemsById<models::compiler::Compiler>>>();
    let playlists_by_id =
        use_context::<Resource<models::MusicItemsById<models::playlist::PlayList>>>();

    // Debounce the search input
    use_effect(move || {
        let search_value = search_input();
        let current_gen = *generation.peek() + 1;
        generation.set(current_gen);

        spawn(async move {
            async_std::task::sleep(Duration::from_secs(2)).await;
            // Only update if this is still the latest generation
            let latest_gen = *generation.peek();
            if latest_gen == current_gen {
                debounced_search.set(search_value);
            }
        });
    });

    let search_resource = use_resource(move || async move {
        let query = debounced_search();
        if query.is_empty() {
            return None;
        }
        match crate::do_search(query).await {
            Ok(result) => Some(result),
            Err(e) => {
                tracing::error!("Search failed: {:?}", e);
                None
            }
        }
    });

    rsx! {
        match (&*compilers_by_id.read_unchecked(), &*playlists_by_id.read_unchecked()) {
            (Some(_), Some(_)) => rsx! {
                div {
                    class: "max-w-full lg:w-6xl mx-auto flex flex-col",
                    style: "gap: 1rem; margin-top: 1rem;",

                    div { class: "flex flex-row items-center gap-1",
                        svg {
                            xmlns: "http://www.w3.org/2000/svg",
                            view_box: "0 0 16 16",
                            fill: "currentColor",
                            class: "mr-1.5",
                            height: "1.75em",
                            width: "1.75em",
                            path {
                                fill_rule: "evenodd",
                                d: "M9.965 11.026a5 5 0 1 1 1.06-1.06l2.755 2.754a.75.75 0 1 1-1.06 1.06l-2.755-2.754ZM10.5 7a3.5 3.5 0 1 1-7 0 3.5 3.5 0 0 1 7 0Z",
                                clip_rule: "evenodd",
                            }
                        }
                        input {
                            r#type: "text",
                            oninput: move |e| search_input.set(e.value()),
                            placeholder: "Search...",
                            class: "w-full",
                        }
                    }

                    div {
                        match &*search_resource.read_unchecked() {
                            None => rsx! {
                                components::Loading {}
                            },
                            Some(None) => rsx! {},
                            Some(Some(search_data)) => rsx! {
                                div { class: "flex flex-row gap-2 mb-4",
                                    for music_item_type_options in ["Tracks", "Artists", "Playlists", "Compilers"].iter() {
                                        a {
                                            style: "font-weight: bold; border-width: 2px",
                                            class: if selected_music_item_type.read_unchecked().as_str() == *music_item_type_options { "active" },
                                            onclick: move |_| selected_music_item_type.set(music_item_type_options.to_string()),
                                            "{music_item_type_options}"
                                        }
                                    }
                                }

                                match selected_music_item_type.read_unchecked().as_str() {
                                    "Tracks" => {
                                        if search_data.tracks.sorted_track_ids.is_empty() {
                                            rsx! {
                                                p { "Nada." }
                                            }
                                        } else {
                                            rsx! {
                                                TrackListComp {
                                                    track_ids: search_data.tracks.sorted_track_ids.clone(),
                                                    tracks_by_id: Some(search_data.tracks.tracks_by_id.clone()),
                                                    linked_tracks: Some(search_data.tracks.linked_tracks.clone()),
                                                    artists_by_id: Some(search_data.tracks.artists_by_id.clone()),
                                                    albums_by_id: Some(search_data.tracks.albums_by_id.clone()),
                                                }
                                            }
                                        }
                                    }
                                    "Artists" => {
                                        if search_data.artists.is_empty() {
                                            rsx! {
                                                p { "Nada." }
                                            }
                                        } else {
                                            rsx! {
                                                ArtistListComp { artists: search_data.artists.clone() }
                                            }
                                        }
                                    }
                                    "Playlists" => {
                                        if search_data.playlist_ids.is_empty() {
                                            rsx! {
                                                p { "Nada." }
                                            }
                                        } else {
                                            rsx! {
                                                crate::views::playlist::PlaylistListComp { playlist_ids: search_data.playlist_ids.clone(), hide_compiler: false }
                                            }
                                        }
                                    }
                                    "Compilers" => {
                                        if search_data.compiler_ids.is_empty() {
                                            rsx! {
                                                p { "Nada." }
                                            }
                                        } else {
                                            rsx! {
                                                crate::views::compiler::CompilerListComp { compiler_ids: search_data.compiler_ids.clone() }
                                            }
                                        }
                                    }
                                    &_ => rsx! {},
                                }
                            },
                        }
                    }
                }
            },
            _ => rsx! {
                components::Loading {}
            },
        }
    }
}
