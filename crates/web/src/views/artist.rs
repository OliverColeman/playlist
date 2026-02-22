use crate::components;
use crate::views::track::TrackListComp;
use dioxus::prelude::*;
use playlist_core::models;
use playlist_core::models::artist::Artist;
use playlist_core::models::playlist::PlaylistCollection;
use std::collections::HashSet;

#[component]
pub fn ArtistComp(id: String) -> Element {
    let artist_with_associated_data_resource: Resource<models::artist::ArtistWithAssociatedData> =
        use_resource(use_reactive!(|id| async move {
            crate::load_artist_with_associated_data(id.to_string())
                .await
                .unwrap()
        }));

    let playlists_by_id =
        use_context::<Resource<models::MusicItemsById<models::playlist::PlayList>>>();

    let track_ids_in_playlists: HashSet<String> = playlists_by_id
        .read_unchecked()
        .as_ref()
        .map(|playlists| {
            playlists
                .sorted_by_date(-1)
                .into_iter()
                .flat_map(|playlist| playlist.track_ids)
                .collect()
        })
        .unwrap_or_default();

    rsx! {
        div { class: "max-w-full lg:w-6xl mx-auto",
            div {
                match (

                    &*artist_with_associated_data_resource.read_unchecked(),
                    &*playlists_by_id.read_unchecked(),
                ) {
                    (None, _) => rsx! {

                        h1 {
                            "Artist: "
                            components::Loading {}
                        }
                    },
                    (Some(artist_data), playlists_option) => {
                        let mut filtered_track_ids: Vec<String> = artist_data
                            .tracks_by_id
                            .keys()
                            .filter(|track_id| track_ids_in_playlists.contains(*track_id))
                            .cloned()
                            .collect();

                        filtered_track_ids

                            .sort_by(|a, b| {
                                let name_a = artist_data.tracks_by_id.get(a).map(|t| &t.name);
                                let name_b = artist_data.tracks_by_id.get(b).map(|t| &t.name);
                                name_a.cmp(&name_b)
                            });
                        let artist = artist_data.artists_by_id.get(&id);
                        rsx! {
                            match artist {
                                Some(artist) => rsx! {
                                    div { class: "flex justify-between gap-10",
                                        div { class: "flex flex-wrap items-center gap-x-6",
                                            h1 {
                                                "Artist: "
                                                span { class: "value", "{&artist.name}" }
                                            }
                                            components::ServiceLinks { music_item: artist.clone() }
                                        }
                                        components::MusicItemImage { music_item: artist.clone() }
                                    }
                                },
                                None => rsx! {
                                    h1 {
                                        "Artist: "
                                        components::Loading {}
                                    }
                                },
                            }

                            div { class: "mt-[1em] md:mt-[2em]",
                                h2 { "Tracks:" }
                                if filtered_track_ids.is_empty() {
                                    if playlists_option.is_none() {
                                        components::Loading {}
                                    } else {
                                        p { "No tracks from this artist appear in any playlists." }
                                    }
                                } else {
                                    TrackListComp {
                                        track_ids: filtered_track_ids,
                                        tracks_by_id: Some(artist_data.tracks_by_id.clone()),
                                        linked_tracks: Some(artist_data.linked_tracks.clone()),
                                        artists_by_id: Some(artist_data.artists_by_id.clone()),
                                        albums_by_id: Some(artist_data.albums_by_id.clone()),
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn ArtistListComp(artists: Vec<Artist>) -> Element {
    rsx! {
        table { class: "table-fixed w-full",
            tbody {
                for artist in artists.iter() {
                    tr { key: "{artist.id}",
                        td {
                            Link { to: format!("/artist/{}", artist.id), "{artist.name}" }
                        }
                        td {
                            components::ServiceLinks { music_item: artist.clone() }
                        }
                    }
                }
            }
        }
    }
}
