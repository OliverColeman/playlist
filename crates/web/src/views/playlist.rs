use crate::components;
use playlist_core::models;
use playlist_core::models::MusicItemCollection;
use playlist_core::models::playlist::PlaylistCollection;
use crate::views::track::TrackListComp;
use dioxus::prelude::*;

#[component]
pub fn PlaylistComp(id: String) -> Element {
    let playlist_data_resource = use_resource(use_reactive!(|id| async move {
        crate::load_playlist_with_associated_data(id.to_string())
            .await
            .unwrap()
    }));

    let playlists_by_id =
        use_context::<Resource<models::MusicItemsById<models::playlist::PlayList>>>();

    let compilers_by_id =
        use_context::<Resource<models::MusicItemsById<models::compiler::Compiler>>>();

    rsx! {
        div { class: "max-w-full lg:w-6xl mx-auto",
            div {

                match &*playlist_data_resource.read_unchecked() {
                    None => rsx! {
                        h1 { components::Loading {} }
                    },
                    Some(playlist_data) => rsx! {
                        h1 {

        

                            "Playlist: "
                            span { class: "value", "{playlist_data.playlist.name}" }
                        }
                        div { class: "flex flex-col md:flex-row gap-[0.3em] md:gap-[2em] md:items-center",
        
                            div { class: "flex",
        
                                match playlist_data.playlist.date {
                                    Some(date) => rsx! {
                                        h6 { "Date:" }
                                        div { {crate::util::format_date(date)} }
                                    },
                                    None => rsx! {},
                                }
                            }
                            div { class: "flex",
                                h6 { "Length:" }
                                div {
                                    {crate::util::format_duration(playlist_data.playlist.duration)}
                                    ", "
                                    {playlist_data.playlist.track_ids.len().to_string()}
                                    " tracks"
                                }
                            }
                            div { class: "flex items-center",
                                h6 { "Compiler(s):" }
                                div {
                                    match (&*playlists_by_id.read_unchecked(), &*compilers_by_id.read_unchecked()) {
                                        (Some(playlists_by_id), Some(compilers_by_id)) => {
                                            match playlists_by_id.get(&playlist_data.playlist.id) {
                                                Some(playlist) => {
                                                    rsx! {
                                                        for compiler_id in playlist.compiler_ids.iter() {
                                                            match compilers_by_id.get(compiler_id) {
                                                                Some(compiler) => rsx! {
                                                                    Link { key: "{compiler.id}", to: "/compiler/{compiler.id}", "{compiler.name} " }
                                                                },
                                                                None => rsx! {},
                                                            }
                                                        }
                                                    }
                                                }
                                                None => rsx! { "-" },
                                            }
                                        }
                                        _ => rsx! {
                                            components::Loading {}
                                        },
                                    }
                                }
                            }
                        }
        
                        match &playlist_data.playlist.notes {
                            Some(notes) => rsx! {
                                div { class: "max-h-[6em] overflow-y-auto [mask-image:linear-gradient(to_bottom,black_calc(100%-1.5em),transparent)]",
                                    pre { class: "pb-[1.5em]", "{notes}" }
                                }
                            },
                            None => rsx! {},
                        }
        
                        div { class: "mt-[1em] md:mt-[2em]",
                            TrackListComp {
                                track_ids: playlist_data.playlist.track_ids.clone(),
                                tracks_by_id: playlist_data.tracks_by_id.clone(),
                                linked_tracks: playlist_data.linked_tracks.clone(),
                                artists_by_id: playlist_data.artists_by_id.clone(),
                                albums_by_id: playlist_data.albums_by_id.clone(),
                            }
                        }
                    },
                }
            }
        }
    }
}

#[component]
pub fn PlaylistListComp(
    playlists: models::MusicItemsById<models::playlist::PlayList>,
    hide_compiler: bool,
) -> Element {
    let compilers_by_id =
        use_context::<Resource<models::MusicItemsById<models::compiler::Compiler>>>();

    rsx! {
        table { class: "playlist-list",
            thead {
                tr {
                    th { "Name" }
                    th { "Date" }
                    if !hide_compiler {
                        th { "Compiler(s)" }
                    }
                    th { class: "hidden lg:table-cell", "Length" }
                    th { class: "collapse", "Icons" }
                
                }
            }
            tbody {
                for playlist in playlists.sorted_by_date(-1).into_iter() {
                    tr { key: "{playlist.id}",
                        td {
                            Link {
                                class: "max-w-[7em] truncate text-ellipsis",
                                to: "/playlist/{playlist.id}",
                                title: "{playlist.name}",
                                "{playlist.name}"
                            }
                        }
                        td {
                            match playlist.date {
                                Some(date) => crate::util::format_date(date),
                                None => "N/A".to_string(),
                            }
                        }

                        if !hide_compiler {
                            td {
                                match &*compilers_by_id.read_unchecked() {
                                    Some(map) => {
                                        rsx! {
                                            for compiler_id in playlist.compiler_ids.iter() {
                                                match map.get(compiler_id) {
                                                    Some(compiler) => rsx! {
                                                        Link { key: "{compiler.id}", to: "/compiler/{compiler.id}", "{compiler.name} " }
                                                    },
                                                    None => rsx! {},
                                                }
                                            }
                                        }
                                    }
                                    None => rsx! {
                                        span { components::Loading {} }
                                    },
                                }
                            }
                        }
                        td { class: "hidden lg:table-cell",
                            "{crate::util::format_duration(playlist.duration)}, "
                            "{playlist.track_ids.len()} tracks"
                        }
                        td { components::IconsAndLinks {} }
                    }
                }
            
            }
        }
    }
}
