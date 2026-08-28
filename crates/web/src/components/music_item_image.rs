use dioxus::prelude::*;
use playlist_core::models::{ExternalServiceAssociation, MusicItem};

#[component]
pub fn MusicItemImage<T: MusicItem + PartialEq + 'static>(music_item: T) -> Element {
    let image_urls: Option<Option<playlist_core::models::ImageUrls>> = music_item
        .external_service_associations()
        .and_then(|associations| {
            associations.iter().find_map(|assoc| match assoc {
                ExternalServiceAssociation::Spotify { image_urls, .. }
                | ExternalServiceAssociation::Tidal { image_urls, .. } => Some(image_urls.clone()),
                ExternalServiceAssociation::MusicBrainz { .. } => None,
            })
        });

    match image_urls {
        Some(image_urls) => {
            let image_url = image_urls
                .and_then(|urls| urls.large.or_else(|| urls.medium).or_else(|| urls.small));
            match image_url {
                Some(url) => rsx! {
                    img {
                        class: "size-24 sm:size-32 md:size-48 object-cover rounded",
                        src: "{url}",
                        alt: "Image of {music_item_type_to_string(T::item_type())} {music_item.name()}",
                    }
                },
                None => rsx! {},
            }
        }
        None => rsx! {},
    }
}

fn music_item_type_to_string(item_type: playlist_core::models::ItemType) -> &'static str {
    match item_type {
        playlist_core::models::ItemType::Track => "track",
        playlist_core::models::ItemType::Artist => "artist",
        playlist_core::models::ItemType::Album => "album",
        playlist_core::models::ItemType::Playlist => "playlist",
        playlist_core::models::ItemType::Compiler => "playlist compiler",
    }
}
