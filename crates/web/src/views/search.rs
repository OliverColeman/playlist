use crate::components;
use crate::views::track::TrackListComp;
use dioxus::prelude::*;
use std::time::Duration;

#[component]
pub fn SearchComp() -> Element {
    let mut search_input = use_signal(|| "".to_string());
    let mut debounced_search = use_signal(|| "".to_string());
    let mut generation = use_signal(|| 0u32);

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
                        if search_data.tracks.sorted_track_ids.is_empty() {
                            p { "Nada." }
                        } else {
                            TrackListComp {
                                track_ids: search_data.tracks.sorted_track_ids.clone(),
                                tracks_by_id: Some(search_data.tracks.tracks_by_id.clone()),
                                linked_tracks: Some(search_data.tracks.linked_tracks.clone()),
                                artists_by_id: Some(search_data.tracks.artists_by_id.clone()),
                                albums_by_id: Some(search_data.tracks.albums_by_id.clone()),
                            }
                        }
                    },
                }
            }
        }
    }
}
