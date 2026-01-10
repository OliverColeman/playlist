use crate::components;
use crate::music_data;
use crate::music_data::MusicItemCollection;
use crate::music_data::playlist::PlaylistCollection;
use dioxus::prelude::*;

#[component]
pub fn PlaylistComp(id: String) -> Element {
    let playlist_data_resource = use_resource(use_reactive!(|id| async move {
        music_data::playlist::load_playlist_with_associated_data(id.to_string())
            .await
            .unwrap()
    }));

    let playlists_by_id =
        use_context::<Resource<music_data::MusicItemsById<music_data::playlist::PlayList>>>();

    let compilers_by_id =
        use_context::<Resource<music_data::MusicItemsById<music_data::compiler::Compiler>>>();

    rsx! {
        div { class: "max-w-full lg:w-6xl mx-auto",
            div {

                match &*playlist_data_resource.read_unchecked() {
                    None => rsx! {
                        h1 { components::Loading {} }
                    },
                    Some(playlist_data) => rsx! {
                        div { class: "flex flex-col md:flex-row gap-[0.1em] md:gap-[3em] lg:items-center",
                            h1 { class: "mb-0", "{playlist_data.playlist.name}" }
                            div { class: "flex lg:flex-row md:flex-col sm:flex-row lg:gap-2",



                                match playlist_data.playlist.date {
                                    Some(date) => rsx! {
                                        h6 { "Date:" }
                                        div { {crate::util::format_date(date)} }
                                    },
                                    None => rsx! {},
                                }
                            }
                            div { class: "flex lg:flex-row md:flex-col sm:flex-row lg:gap-2",
                                h6 { "Length:" }
                                div {
                                    {crate::util::format_duration(playlist_data.playlist.duration)}
                                    ", "
                                    {playlist_data.playlist.track_ids.len().to_string()}
                                    " tracks"
                                }
                            }
                            div { class: "flex lg:flex-row md:flex-col sm:flex-row items-center md:items-start lg:items-center lg:gap-2",
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

                        table {
                            thead {
                                tr {
                                    th { "Title" }
                                    th { "Artist" }
                                    th { class: "hidden lg:table-cell", "Album" }
                                    th { class: "hidden md:table-cell w-[4.5em]", "Length" }
                                    th { class: "hidden sm:table-cell w-[8em]", "Last list" }
                                    th { class: "hidden", "Icons" }
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
                                                td { class: "font-semibold", "{track.name}" }
                                                td {
                                                    for artist_id in track.artist_ids.iter() {
                                                        match playlist_data.artists_by_id.get(artist_id) {
                                                            Some(artist) => rsx! {
                                                                Link { key: "{artist.id}", to: "/artist/{artist.id}", "{artist.name}" }
                                                            },
                                                            None => rsx! {},
                                                        }
                                                    }
                                                }
                                                td { class: "hidden lg:table-cell",
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
                                                td { class: "hidden md:table-cell",
                                                    {crate::util::format_duration(track.duration.unwrap_or_default())}
                                                }
                                                td { class: "hidden sm:table-cell",
                                                    match &*playlists_by_id.read_unchecked() {
                                                        Some(playlists_map) => {
                                                            let most_recent = playlists_map
                                                                .sorted_by_date(-1)
                                                                .into_iter()
                                                                .filter(|p| p.track_ids.contains(&track.id))
                                                                .next();


                                                            match most_recent {
                                                                Some(playlist) => rsx! {
                                                                    Link { to: "/playlist/{playlist.id}", "{playlist.name}" }
                                                                },
                                                                None => rsx! { "-" },
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
