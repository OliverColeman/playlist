use std::collections::{HashMap, HashSet};

use crate::components;
use crate::music_data;
use crate::music_data::album::Album;
use crate::music_data::artist::Artist;
use crate::music_data::playlist;
use crate::music_data::playlist::PlaylistCollection;
use crate::music_data::track::Track;
use crate::views::playlist::PlaylistListComp;
use dioxus::prelude::*;

#[component]
pub fn TrackComp(id: String) -> Element {
    let track_with_associated_data_resource = use_resource(use_reactive!(|id| async move {
        music_data::track::load_track_with_associated_data(id.to_string())
            .await
            .unwrap()
    }));

    let playlists_by_id_resource =
        use_context::<Resource<music_data::MusicItemsById<music_data::playlist::PlayList>>>();

    let playlists_for_track: Option<Vec<playlist::PlayList>> = match (
        &*playlists_by_id_resource.read_unchecked(),
        &*track_with_associated_data_resource.read_unchecked(),
    ) {
        (Some(playlists), Some(track_data)) => Some(
            playlists
                .sorted_by_date(-1)
                .iter()
                .filter(|pl| {
                    track_data
                        .linked_tracks_by_id
                        .values()
                        .any(|t| pl.track_ids.contains(&t.id))
                })
                .cloned()
                .collect::<Vec<playlist::PlayList>>()
                .into(),
        ),
        _ => None,
    };

    rsx! {
        div { class: "max-w-full lg:w-6xl mx-auto",
            div {
                match &*track_with_associated_data_resource.read_unchecked() {
                    None => rsx! {
                        h1 {
                            "Track: "



                            components::Loading {}
                        }
                    },

                    Some(track_data) => rsx! {
                        match track_data.linked_tracks_by_id.get(&id) {
                            None => rsx! {
                                h1 { "Track not found" }
                            },
                            Some(track) => rsx! {
                                h1 {
                                    "Track: "
                                    span { class: "value", "{track.name}" }
                                }
                                div { class: "flex flex-col md:flex-row gap-[0.3em] md:gap-[2em] md:items-center",



                                    div { class: "flex items-center",
                                        h6 { "Artist(s):" }
                                        div {
                                            for artist_id in track.artist_ids.iter() {
                                                match track_data.artists_by_id.get(artist_id) {
                                                    Some(artist) => rsx! {
                                                        Link { key: "{artist.id}", to: "/artist/{artist.id}", "{artist.name} " }
                                                    },
                                                    None => rsx! {},
                                                }
                                            }
                                        }
                                    }
                                    div { class: "flex",
                                        h6 { "Album:" }
                                        div {

                                            match &track_data.albums_by_id.get(&track.album_id.clone().unwrap_or_default()) {
                                                Some(album) => rsx! { "{album.name}" },
                                                None => rsx! { "-" },
                                            }
                                        }
                                    }
                                    div { class: "flex",
                                        h6 { "Length:" }

                                        div { {crate::util::format_duration(track.duration.unwrap_or_default())} }
                                    }
                                }
                            },
                        }

                        div { class: "mt-[1em] md:mt-[2em]",
                            h2 { "Appears in playlists:" }
                            match &playlists_for_track {
                                Some(playlists) if !playlists.is_empty() => rsx! {
                                    PlaylistListComp {
                                        playlists: music_data::MusicItemsById::from(playlists.clone()),
                                        hide_compiler: false,
                                    }
                                },
                                Some(_) => rsx! {
                                    p { "Nada" }
                                },
                                None => rsx! {
                                    components::Loading {}
                                },
                            }
                        }

                        div { class: "mt-[1em] md:mt-[2em]",
                            h2 { "All versions of this track:" }
                            TrackListComp {
                                track_ids: track_data.linked_tracks_by_id.keys().cloned().collect(),
                                tracks_by_id: Some(track_data.linked_tracks_by_id.clone()),
                                linked_tracks: Some(
                                    track_data
                                        .linked_tracks_by_id
                                        .values()
                                        .map(|t| {
                                            let mut set = HashSet::new();
                                            set.insert(t.id.clone());
                                            set
                                        })
                                        .collect(),
                                ),
                                artists_by_id: Some(track_data.artists_by_id.clone()),
                                albums_by_id: Some(track_data.albums_by_id.clone()),
                            }
                        }
                    },
                }
            }
        }
    }
}

#[component]
pub fn TrackListComp(
    track_ids: Vec<String>,
    tracks_by_id: Option<HashMap<String, Track>>,
    linked_tracks: Option<Vec<HashSet<String>>>,
    artists_by_id: Option<HashMap<String, Artist>>,
    albums_by_id: Option<HashMap<String, Album>>,
) -> Element {
    let playlists_by_id =
        use_context::<Resource<music_data::MusicItemsById<music_data::playlist::PlayList>>>();

    rsx! {
        table { class: "table-fixed w-full",
            thead {
                tr {
                    th { "Title" }
                    th { "Artist" }
                    th { class: "hidden lg:table-cell", "Album" }
                    th { class: "hidden md:table-cell w-[4.5em]", "Length" }
                    th {
                        class: "hidden sm:table-cell w-[9em]",
                        title: "How many playlists this track appears in / Most recent playlist containing this track",
                        "List count /"
                        br {}
                        "Most recent"
                    }
                    th { class: "invisible w-[110px]", "Icons" }
                }
            }
            tbody {
                for track_id in track_ids.iter() {
                    match tracks_by_id.as_ref().and_then(|map| map.get(track_id)) {
                        None => rsx! {
                            tr {
                                td { components::Loading {} }
                            }
                        },
                        Some(track) => rsx! {
                            tr { key: "{track.id}",
                                td {



                                    Link { to: "/track/{track.id}", "{track.name}" }
                                }
                                td {
                                    for artist_id in track.artist_ids.iter() {
                                        match artists_by_id.as_ref().and_then(|map| map.get(artist_id)) {
                                            Some(artist) => rsx! {
                                                Link { key: "{artist.id}", to: "/artist/{artist.id}", "{artist.name}" }
                                            },
                                            None => rsx! {


                                                components::Loading {}
                                            },
                                        }
                                    }
                                }

                                td { class: "hidden lg:table-cell",
                                    match &track.album_id {
                                        Some(album_id) => {
                                            match albums_by_id.as_ref().and_then(|map| map.get(album_id)) {
                                                Some(album) => rsx! { "{album.name}" },
                                                None => rsx! {
                                                    components::Loading {}
                                                },
                                            }
                                        }
                                        None => rsx! {},
                                    }
                                }
                                td { class: "hidden md:table-cell",
                                    {crate::util::format_duration(track.duration.unwrap_or_default())}
                                }
                                td {
                                    class: "hidden sm:table-cell",
                                    title: "How many playlists this track appears in / Most recent playlist containing this track",
                                    match &*playlists_by_id.read_unchecked() {
                                        Some(playlists_map) => {
                                            let singular_set = HashSet::from([track.id.clone()]);
                                            let linked_tracks_for_this_track = match linked_tracks.as_ref() {
                                                Some(lt_vec) => {
                                                    lt_vec
                                                        .iter()
                                                        .find(|lt_set| lt_set.contains(&track.id))
                                                        .cloned()
                                                        .unwrap_or(singular_set.clone())
                                                }
                                                None => singular_set.clone(),
                                            };
                                            let matching_playlists: Vec<_> = playlists_map
                                                .sorted_by_date(-1)
                                                .into_iter()
                                                .filter(|p| {
                                                    p.track_ids
                                                        .clone()
                                                        .into_iter()
                                                        .any(|t_id| linked_tracks_for_this_track.contains(&t_id))
                                                })
                                                .collect();
                                            let count = matching_playlists.len();
                                            let most_recent = matching_playlists.first();
                                            rsx! {
                                                "{count} / "
                                                match most_recent {
                                                    Some(playlist) => rsx! {
                                                        Link { to: "/playlist/{playlist.id}", "{playlist.name}" }
                                                    },
                                                    None => rsx! { "-" },
                                                }
                                            }
                                        }
                                        None => rsx! {
                                            components::Loading {}
                                        },
                                    }
                                }
                                td { "TODO" }
                            }
                        },
                    }
                }
            }
        }
    }
}
