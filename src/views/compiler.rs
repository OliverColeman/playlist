use crate::components;
use crate::music_data;
use crate::music_data::MusicItemCollection;
use crate::music_data::playlist;
use crate::music_data::playlist::PlaylistCollection;
use crate::views::playlist::PlaylistListComp;
use dioxus::prelude::*;

#[component]
pub fn CompilerComp(id: String) -> Element {
    let compilers_by_id =
        use_context::<Resource<music_data::MusicItemsById<music_data::compiler::Compiler>>>();
    let playlists_by_id =
        use_context::<Resource<music_data::MusicItemsById<music_data::playlist::PlayList>>>();

    let compilers_read = compilers_by_id.read_unchecked();
    let compiler = match &*compilers_read {
        Some(compilers) => compilers.get(&id),
        None => None,
    };

    let playlists_for_compiler: Option<Vec<playlist::PlayList>> =
        match &*playlists_by_id.read_unchecked() {
            None => None,
            Some(playlists) => match compiler {
                None => None,
                Some(compiler) => Some(
                    playlists
                        .sorted_by_date(-1)
                        .iter()
                        .filter(|pl| pl.compiler_ids.contains(&compiler.id))
                        .cloned()
                        .collect::<Vec<playlist::PlayList>>()
                        .into(),
                ),
            },
        };

    rsx! {

        div { class: "lg:w-4xl mx-auto",
            div {
                h1 {
                    match &compiler {
                        Some(compiler) => rsx! { "{compiler.name}'s Playlists" },
                        None => rsx! {
                            components::Loading {}
                        },
                    }
                }

                match &playlists_for_compiler {
                    Some(playlists) => rsx! {
                        PlaylistListComp {
                            playlists: music_data::MusicItemsById::from(playlists.clone()),
                            hide_compiler: true,
                        }
                    },
                    None => rsx! {
                        components::Loading {}
                    },
                }
            }
        }
    }
}

#[component]
pub fn CompilerListComp() -> Element {
    let compilers_by_id =
        use_context::<Resource<music_data::MusicItemsById<music_data::compiler::Compiler>>>();
    let playlists_by_id =
        use_context::<Resource<music_data::MusicItemsById<music_data::playlist::PlayList>>>();

    let playlists_by_compiler: Option<
        std::collections::HashMap<String, Vec<music_data::playlist::PlayList>>,
    > = match &*playlists_by_id.read_unchecked() {
        None => None,
        Some(playlists) => {
            let mut map: std::collections::HashMap<String, Vec<music_data::playlist::PlayList>> =
                std::collections::HashMap::new();
            for playlist in playlists.sorted_by_date(-1) {
                for compiler_id in &playlist.compiler_ids {
                    map.entry(compiler_id.clone())
                        .or_default()
                        .push(playlist.clone());
                }
            }
            Some(map)
        }
    };

    let headers = vec!["Name", "Playlists", "Icons"];

    rsx! {
        div { class: "lg:w-4xl mx-auto",
            div {
                h1 { "Playlist Compilers" }
                match &*compilers_by_id.read_unchecked() {
                    Some(compilers) => rsx! {
                        table {
                            thead {
                                tr {
                                    for header in headers.iter() {
                                        th { "{header}" }
                                    }
                                }
                            }
                            tbody {
                                for compiler in compilers.sorted_by_name_normalised(1).into_iter() {
                                    tr { key: "{compiler.id}",
                                        td { class: "w-1/4 md:w-3xs",
                                            Link { to: "/compiler/{compiler.id}", "{compiler.name}" }
                                        }
                                        td {
                                            match &playlists_by_compiler {
                                                Some(map) => {
                                                    match map.get(&compiler.id) {
                                                        Some(playlists) => rsx! {
                                                            for playlist in playlists.iter() {
                                                                Link { key: "{playlist.id}", to: "/playlist/{playlist.id}", "{playlist.name} " }
                                                            }
                                                        },
                                                        None => rsx! {
                                                            span { "<Nada>" }
                                                        },
                                                    }
                                                }
                                                None => rsx! {
                                                    span { components::Loading {} }
                                                },
                                            }
                                        }
                                        td { components::IconsAndLinks {} }
                                    }
                                }
                            }
                        }
                    },
                    None => rsx! {
                        h1 { components::Loading {} }
                    },
                }
            }
        }
    }
}
