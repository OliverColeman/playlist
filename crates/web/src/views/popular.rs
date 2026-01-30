use crate::components;
use crate::views::track::TrackListComp;
use dioxus::prelude::*;

#[component]
pub fn PopularTracksComp() -> Element {
    let popular_tracks_resource =
        use_resource(|| async move { crate::load_popular_tracks().await.ok() });

    rsx! {
        div { class: "max-w-full lg:w-6xl mx-auto",
            h1 { "Top 100" }
            div {
                match &*popular_tracks_resource.read_unchecked() {
                    None => rsx! {
                        components::Loading {}
                    },
                    Some(None) => rsx! {
                        p { "Failed to load popular tracks" }
                    },
                    Some(Some(popular_data)) => rsx! {
                        if popular_data.sorted_track_ids.is_empty() {
                            p { "No popular tracks found" }
                        } else {
                            TrackListComp {
                                track_ids: popular_data.sorted_track_ids.clone(),
                                tracks_by_id: Some(popular_data.tracks_by_id.clone()),
                                linked_tracks: Some(popular_data.linked_tracks.clone()),
                                artists_by_id: Some(popular_data.artists_by_id.clone()),
                                albums_by_id: Some(popular_data.albums_by_id.clone()),
                            }
                        }
                    },
                }
            }
        }
    }
}
