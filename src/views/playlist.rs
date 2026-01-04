use crate::components;
use crate::music_data;
use crate::music_data::MusicItemCollection;
use crate::music_data::playlist::PlaylistCollection;
use dioxus::html::th;
use dioxus::prelude::*;

#[component]
pub fn PlaylistComp(id: String) -> Element {
    let playlist_data_resource = use_resource(move || {
        let id = id.clone();
        async move {
            music_data::playlist::load_playlist_with_associated_data(id)
                .await
                .unwrap()
        }
    });

    rsx! {
        div { class: "lg:w-6xl mx-auto",
            div {

                match &*playlist_data_resource.read_unchecked() {
                    None => rsx! {
                        h1 { components::Loading {} }
                    },
                    Some(playlist_data) => rsx! {
                        h1 { "{playlist_data.playlist.name}" }
                        table {
                            thead {
                                tr {
                                    th { "Title" }
                                    th { "Artist" }
                                    th { "Album" }
                                    th { "Length" }
                                    th { "Last list" }
                                    th { "Icons" }
                                }
                            }
                            tbody {
                                for track_id in playlist_data.playlist.track_ids.iter() {
                                    match playlist_data.tracks_by_id.get(track_id) {
                                        None => rsx! {
                                            tr {
                                                td { "<data missing>" }
                                            }
                                        },
                                        Some(track) => rsx! {
                                            tr { key: "{track.id}",
                                                td { "{track.name}" }
                                                td {
                                                    for artist_id in track.artist_ids.iter() {
                                                        match playlist_data.artists_by_id.get(artist_id) {
                                                            Some(artist) => rsx! {
                                                                a { key: "{artist.id}", href: "/artist/{artist.id}", "{artist.name}" }
                                                            },
                                                            None => rsx! {},
                                                        }
                                                    }
                                                }
                                                td {
                                                    match &track.album_id {
                                                        Some(album_id) => {
                                                            match playlist_data.albums_by_id.get(album_id) {
                                                                Some(album) => rsx! { "{album.name}" },
                                                                None => rsx! {},
                                                            }
                                                        }
                                                        None => rsx! {},
                                                    }
                                                }
                                                td { {crate::util::format_duration(track.duration.unwrap_or_default())} }
                                            }
                                        },
                                    }
                                }
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
    playlists: music_data::MusicItemsById<music_data::playlist::PlayList>,
    hide_compiler: bool,
) -> Element {
    let compilers_by_id =
        use_context::<Resource<music_data::MusicItemsById<music_data::compiler::Compiler>>>();

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
                            a {
                                class: "max-w-[7em] truncate text-ellipsis",
                                href: "/playlist/{playlist.id}",
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
                                                        a { key: "{compiler.id}", href: "/compiler/{compiler.id}", "{compiler.name} " }
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
